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

// getExprName extracts a readable name from an AST expression (e.g., "fmt.Println")
func getExprName(expr ast.Expr) string {
	switch e := expr.(type) {
	case *ast.Ident:
		return e.Name
	case *ast.SelectorExpr:
		return getExprName(e.X) + "." + e.Sel.Name
	default:
		return "unknown"
	}
}

// getFuncPrefix resolves the correct prefix (Package or Package.Struct) for a function
func getFuncPrefix(pkgName string, decl *ast.FuncDecl) string {
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
	return prefix
}

// SymbolTable maintains the global index of symbols and local file imports
type SymbolTable struct {
	symbols     map[string]uint64
	fileImports map[string]map[string]string
}

func NewSymbolTable() *SymbolTable {
	return &SymbolTable{
		symbols:     make(map[string]uint64),
		fileImports: make(map[string]map[string]string),
	}
}

// ParsedFile caches the AST and metadata for Pass 2
type ParsedFile struct {
	Path    string
	PkgName string
	Tree    *ast.File
	Fset    *token.FileSet
}

// discoverSymbols (Pass 1) harvests imports and function declarations into the SymbolTable
func discoverSymbols(file *ParsedFile, table *SymbolTable) {
	localImports := make(map[string]string)
	for _, imp := range file.Tree.Imports {
		path := strings.Trim(imp.Path.Value, "\"`")
		alias := filepath.Base(path)
		if imp.Name != nil {
			alias = imp.Name.Name
		}
		localImports[alias] = path
	}
	table.fileImports[file.Path] = localImports

	for _, decl := range file.Tree.Decls {
		if fnDecl, ok := decl.(*ast.FuncDecl); ok {
			fnName := fnDecl.Name.Name
			prefix := getFuncPrefix(file.PkgName, fnDecl)
			fqsn := fmt.Sprintf("go:%s/%s", prefix, fnName)
			table.symbols[fqsn] = hashId(fqsn)
		}
	}
}

// emitFacts (Pass 2) walks the AST again, generating module, function, and call-site edges
func emitFacts(file *ParsedFile, table *SymbolTable) []string {
	var facts []string
	pkgName := file.PkgName
	filename := file.Path

	// 1. Emit Module (File/Package) Entity
	modFqsn := fmt.Sprintf("go:%s/%s", pkgName, filepath.Base(filename))
	modId := hashId(modFqsn)

	modFact := fmt.Sprintf("[[:db/add %d :entity/type :type/module] [:db/add %d :entity/language :lang/go] [:db/add %d :symbol/fqsn \"%s\"] [:db/add %d :location/file \"%s\"]]",
		modId, modId, modId, escapeString(modFqsn), modId, escapeString(filename))
	facts = append(facts, modFact)

	localImports := table.fileImports[filename]

	// 2. Walk the AST to find Functions and Call Sites
	ast.Inspect(file.Tree, func(n ast.Node) bool {
		switch decl := n.(type) {
		case *ast.FuncDecl:
			fnName := decl.Name.Name
			prefix := getFuncPrefix(pkgName, decl)

			fnFqsn := fmt.Sprintf("go:%s/%s", prefix, fnName)
			fnId := hashId(fnFqsn)
			pos := file.Fset.Position(decl.Pos())

			fact := fmt.Sprintf("[[:db/add %d :entity/type :type/function] [:db/add %d :entity/language :lang/go] [:db/add %d :symbol/fqsn \"%s\"] [:db/add %d :symbol/name \"%s\"] [:db/add %d :parent/entity %d] [:db/add %d :location/file \"%s\"] [:db/add %d :location/start-line %d] [:db/add %d :location/start-col %d]",
				fnId, fnId, fnId, escapeString(fnFqsn), fnId, escapeString(fnName), fnId, modId, fnId, escapeString(filename), fnId, pos.Line, fnId, pos.Column)

			if decl.Doc != nil {
				docText := decl.Doc.Text()
				fact += fmt.Sprintf(" [:db/add %d :doc/text \"%s\"]", fnId, escapeString(strings.TrimSpace(docText)))
			}
			fact += "]"
			facts = append(facts, fact)

			// 3. Inspect the function body for Call Sites
			if decl.Body != nil {
				ast.Inspect(decl.Body, func(bn ast.Node) bool {
					if call, ok := bn.(*ast.CallExpr); ok {
						calleeName := getExprName(call.Fun)
						if calleeName == "unknown" {
							return true
						}

						var calleeFqsn string
						parts := strings.SplitN(calleeName, ".", 2)
						if len(parts) == 2 {
							pkgAlias := parts[0]
							methodName := parts[1]
							if fullPkg, ok := localImports[pkgAlias]; ok {
								calleeFqsn = fmt.Sprintf("go:%s/%s", fullPkg, methodName)
							} else {
								calleeFqsn = fmt.Sprintf("go:%s/%s", pkgAlias, methodName)
							}
						} else {
							calleeFqsn = fmt.Sprintf("go:%s/%s", pkgName, calleeName)
						}

						callPos := file.Fset.Position(call.Pos())
						callSiteFqsn := fmt.Sprintf("call:go:%s:%d:%d:%s->%s", filename, callPos.Line, callPos.Column, fnFqsn, calleeFqsn)
						callSiteId := hashId(callSiteFqsn)
						calleeId := hashId(calleeFqsn)

						callFact := fmt.Sprintf("[[:db/add %d :entity/type :type/call-site] [:db/add %d :call/caller %d] [:db/add %d :call/callee %d] [:db/add %d :location/file \"%s\"] [:db/add %d :location/start-line %d] [:db/add %d :location/start-col %d]]",
							callSiteId, callSiteId, fnId, callSiteId, calleeId, callSiteId, escapeString(filename), callSiteId, callPos.Line, callSiteId, callPos.Column)
						facts = append(facts, callFact)
					}
					return true
				})
			}
		}
		return true
	})

	return facts
}

// RunTwoPass orchestrates the two-pass extraction logic over a slice of file paths
func RunTwoPass(fset *token.FileSet, paths []string) ([]string, error) {
	table := NewSymbolTable()
	var parsedFiles []*ParsedFile

	// Pass 1: Parse and Discover
	for _, path := range paths {
		node, err := parser.ParseFile(fset, path, nil, parser.ParseComments)
		if err != nil {
			return nil, err
		}
		pf := &ParsedFile{
			Path:    filepath.ToSlash(path),
			PkgName: node.Name.Name,
			Tree:    node,
			Fset:    fset,
		}
		discoverSymbols(pf, table)
		parsedFiles = append(parsedFiles, pf)
	}

	// Pass 2: Emit Facts
	var allFacts []string
	for _, pf := range parsedFiles {
		allFacts = append(allFacts, emitFacts(pf, table)...)
	}

	return allFacts, nil
}

func main() {
	if len(os.Args) < 2 {
		fmt.Println("Usage: astrolabe-adapter-go <target_directory>")
		os.Exit(1)
	}
	targetDir := os.Args[1]
	fset := token.NewFileSet()

	var filesToProcess []string
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
			filesToProcess = append(filesToProcess, path)
		}
		return nil
	})

	if facts, err := RunTwoPass(fset, filesToProcess); err == nil {
		for _, fact := range facts {
			fmt.Println(fact)
		}
	} else {
		fmt.Fprintf(os.Stderr, "Error parsing files: %v\n", err)
		os.Exit(1)
	}
}
