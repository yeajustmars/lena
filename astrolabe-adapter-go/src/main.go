// astrolabe-adapter-go/src/main.go

package main

import (
	"fmt"
	"go/ast"
	"go/parser"
	"go/token"
	"hash/fnv"
	"os"
	"path/filepath"
	"strings"
)

func hashId(s string) uint64 {
	h := fnv.New64a()
	h.Write([]byte(s))
	// Mask the most significant bit so it parses safely as a positive i64 in MesoDB
	return h.Sum64() & 0x7FFFFFFFFFFFFFFF
}

func escapeString(s string) string {
	s = strings.ReplaceAll(s, "\\", "\\\\")
	s = strings.ReplaceAll(s, "\"", "\\\"")
	s = strings.ReplaceAll(s, "\n", "\\n")
	s = strings.ReplaceAll(s, "\r", "\\r")
	s = strings.ReplaceAll(s, "\t", "\\t")
	return s
}

// extractFacts decouples parsing from stdout to enable robust unit testing
func extractFacts(fset *token.FileSet, path string) ([]string, error) {
	node, err := parser.ParseFile(fset, path, nil, parser.ParseComments)
	if err != nil {
		return nil, err
	}

	var facts []string
	pkgName := node.Name.Name
	filename := filepath.ToSlash(path)

	// 1. Emit Module (File/Package) Entity
	modFqsn := fmt.Sprintf("go:%s/%s", pkgName, filepath.Base(filename))
	modId := hashId(modFqsn)

	modFact := fmt.Sprintf("[[:db/add %d :entity/type :type/module] [:db/add %d :entity/language :lang/go] [:db/add %d :symbol/fqsn \"%s\"] [:db/add %d :location/file \"%s\"]]",
		modId, modId, modId, escapeString(modFqsn), modId, escapeString(filename))
	facts = append(facts, modFact)

	// 2. Walk the AST to find Functions and Structs
	ast.Inspect(node, func(n ast.Node) bool {
		switch decl := n.(type) {
		case *ast.FuncDecl:
			fnName := decl.Name.Name

			// Handle receiver (methods vs standalone functions)
			prefix := pkgName
			if decl.Recv != nil && len(decl.Recv.List) > 0 {
				if starExpr, ok := decl.Recv.List[0].Type.(*ast.StarExpr); ok {
					if ident, ok := starExpr.X.(*ast.Ident); ok {
						prefix = fmt.Sprintf("%s.%s", pkgName, ident.Name)
					}
				} else if ident, ok := decl.Recv.List[0].Type.(*ast.Ident); ok {
					prefix = fmt.Sprintf("%s.%s", pkgName, ident.Name)
				}
			}

			fnFqsn := fmt.Sprintf("go:%s/%s", prefix, fnName)
			fnId := hashId(fnFqsn)
			pos := fset.Position(decl.Pos())

			fact := fmt.Sprintf("[[:db/add %d :entity/type :type/function] [:db/add %d :entity/language :lang/go] [:db/add %d :symbol/fqsn \"%s\"] [:db/add %d :symbol/name \"%s\"] [:db/add %d :parent/entity %d] [:db/add %d :location/file \"%s\"] [:db/add %d :location/start-line %d] [:db/add %d :location/start-col %d]",
				fnId, fnId, fnId, escapeString(fnFqsn), fnId, escapeString(fnName), fnId, modId, fnId, escapeString(filename), fnId, pos.Line, fnId, pos.Column)

			if decl.Doc != nil {
				docText := decl.Doc.Text()
				fact += fmt.Sprintf(" [:db/add %d :doc/text \"%s\"]", fnId, escapeString(strings.TrimSpace(docText)))
			}
			fact += "]"
			facts = append(facts, fact)
		}
		return true
	})

	return facts, nil
}

func main() {
	if len(os.Args) < 2 {
		fmt.Println("Usage: astrolabe-adapter-go <target_directory>")
		os.Exit(1)
	}
	targetDir := os.Args[1]
	fset := token.NewFileSet()

	filepath.WalkDir(targetDir, func(path string, d os.DirEntry, err error) error {
		if err != nil {
			return nil
		}
		if d.IsDir() {
			name := d.Name()
			if name == ".git" || name == "vendor" || name == "node_modules" {
				return filepath.SkipDir
			}
		} else if strings.HasSuffix(d.Name(), ".go") {
			if facts, err := extractFacts(fset, path); err == nil {
				for _, fact := range facts {
					fmt.Println(fact)
				}
			}
		}
		return nil
	})
}
