// lena-adapter-go/src/main.go

package main

import (
	"bytes"
	"flag"
	"fmt"
	"go/ast"
	"go/parser"
	"go/printer"
	"go/token"
	"hash/fnv"
	"os"
	"path/filepath"
	"strings"
)

const AdapterVersion = "0.2.0"

func hashId(s string) uint64 {
	h := fnv.New64a()
	h.Write([]byte(s))
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

// getContainer returns the receiver struct name if the function is a method.
func getContainer(decl *ast.FuncDecl) string {
	if decl.Recv != nil && len(decl.Recv.List) > 0 {
		if starExpr, ok := decl.Recv.List[0].Type.(*ast.StarExpr); ok {
			if ident, ok := starExpr.X.(*ast.Ident); ok {
				return ident.Name
			}
		} else if ident, ok := decl.Recv.List[0].Type.(*ast.Ident); ok {
			return ident.Name
		}
	}
	return ""
}

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

type ParsedFile struct {
	Path    string
	PkgName string
	Tree    *ast.File
	Fset    *token.FileSet
}

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
			container := getContainer(fnDecl)

			var fqsn string
			if container != "" {
				fqsn = fmt.Sprintf("go:%s/%s.%s", file.PkgName, container, fnName)
			} else {
				fqsn = fmt.Sprintf("go:%s/%s", file.PkgName, fnName)
			}
			table.symbols[fqsn] = hashId(fqsn)
		}
	}
}

func emitFacts(file *ParsedFile, table *SymbolTable) []string {
	var facts []string
	pkgName := file.PkgName
	filename := file.Path

	// 1. Emit Module Entity
	modFqsn := fmt.Sprintf("go:%s", pkgName)
	modId := hashId(modFqsn)

	modFact := fmt.Sprintf("[[:db/add %d :entity/type :type/module] [:db/add %d :entity/language :lang/go] [:db/add %d :symbol/fqsn \"%s\"] [:db/add %d :symbol/module \"%s\"] [:db/add %d :symbol/name \"%s\"] [:db/add %d :location/file \"%s\"]]",
		modId, modId, modId, escapeString(modFqsn), modId, escapeString(pkgName), modId, escapeString(pkgName), modId, escapeString(filename))
	facts = append(facts, modFact)

	localImports := table.fileImports[filename]
	emittedExternals := make(map[uint64]bool)

	// 2. Walk AST for Functions
	ast.Inspect(file.Tree, func(n ast.Node) bool {
		switch decl := n.(type) {
		case *ast.FuncDecl:
			fnName := decl.Name.Name
			container := getContainer(decl)

			var fnFqsn string
			if container != "" {
				fnFqsn = fmt.Sprintf("go:%s/%s.%s", pkgName, container, fnName)
			} else {
				fnFqsn = fmt.Sprintf("go:%s/%s", pkgName, fnName)
			}
			fnId := hashId(fnFqsn)
			pos := file.Fset.Position(decl.Pos())

			var sigBuf bytes.Buffer
			printer.Fprint(&sigBuf, file.Fset, decl.Type)
			sigStr := "func " + fnName + sigBuf.String()

			kind := ":kind/fn"
			if container != "" {
				kind = ":kind/method"
			}

			fact := fmt.Sprintf("[[:db/add %d :entity/type :type/function] [:db/add %d :entity/language :lang/go] [:db/add %d :symbol/fqsn \"%s\"] [:db/add %d :symbol/module \"%s\"] [:db/add %d :symbol/name \"%s\"] [:db/add %d :symbol/kind %s] [:db/add %d :code/signature \"%s\"] [:db/add %d :parent/entity %d] [:db/add %d :location/file \"%s\"] [:db/add %d :location/start-line %d] [:db/add %d :location/start-col %d]",
				fnId, fnId, fnId, escapeString(fnFqsn), fnId, escapeString(pkgName), fnId, escapeString(fnName), fnId, kind, fnId, fnId, escapeString(sigStr), fnId, modId, fnId, escapeString(filename), fnId, pos.Line, fnId, pos.Column)

			if container != "" {
				fact += fmt.Sprintf(" [:db/add %d :symbol/container \"%s\"]", fnId, escapeString(container))
			}

			if decl.Doc != nil {
				docText := decl.Doc.Text()
				fact += fmt.Sprintf(" [:db/add %d :doc/text \"%s\"]", fnId, escapeString(strings.TrimSpace(docText)))
			}
			fact += "]"
			facts = append(facts, fact)

			// 3. Inspect Body for Calls
			if decl.Body != nil {
				ast.Inspect(decl.Body, func(bn ast.Node) bool {
					if call, ok := bn.(*ast.CallExpr); ok {
						calleeName := getExprName(call.Fun)
						if calleeName == "unknown" {
							return true
						}

						var calleeFqsn, calleeMod, calleeTarget string
						isExternal := false
						parts := strings.SplitN(calleeName, ".", 2)

						if len(parts) == 2 {
							pkgAlias := parts[0]
							calleeTarget = parts[1]
							if fullPkg, ok := localImports[pkgAlias]; ok {
								calleeFqsn = fmt.Sprintf("go:%s/%s", fullPkg, calleeTarget)
								calleeMod = fullPkg
								isExternal = !strings.HasPrefix(fullPkg, pkgName) // Rough heuristic; real resolution relies on index
							} else {
								calleeFqsn = fmt.Sprintf("go:%s/%s", pkgAlias, calleeTarget)
								calleeMod = pkgAlias
								isExternal = true
							}
						} else {
							calleeFqsn = fmt.Sprintf("go:%s/%s", pkgName, calleeName)
							calleeTarget = calleeName
							calleeMod = pkgName
						}

						callPos := file.Fset.Position(call.Pos())
						callSiteFqsn := fmt.Sprintf("call:go:%s:%d:%d:%s->%s", filename, callPos.Line, callPos.Column, fnFqsn, calleeFqsn)
						callSiteId := hashId(callSiteFqsn)
						calleeId := hashId(calleeFqsn)

						callFact := fmt.Sprintf("[[:db/add %d :entity/type :type/call-site] [:db/add %d :call/caller %d] [:db/add %d :call/callee %d] [:db/add %d :location/file \"%s\"] [:db/add %d :location/start-line %d] [:db/add %d :location/start-col %d]]",
							callSiteId, callSiteId, fnId, callSiteId, calleeId, callSiteId, escapeString(filename), callSiteId, callPos.Line, callSiteId, callPos.Column)
						facts = append(facts, callFact)

						// Emit external stub if needed
						if isExternal && !emittedExternals[calleeId] {
							stubFact := fmt.Sprintf("[[:db/add %d :symbol/fqsn \"%s\"] [:db/add %d :symbol/name \"%s\"] [:db/add %d :symbol/module \"%s\"] [:db/add %d :symbol/external true]]",
								calleeId, escapeString(calleeFqsn), calleeId, escapeString(calleeTarget), calleeId, escapeString(calleeMod), calleeId)
							facts = append(facts, stubFact)
							emittedExternals[calleeId] = true
						}
					}
					return true
				})
			}
		}
		return true
	})

	return facts
}

func RunTwoPass(fset *token.FileSet, paths []string) ([]string, error) {
	table := NewSymbolTable()
	var parsedFiles []*ParsedFile

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

	var allFacts []string
	for _, pf := range parsedFiles {
		allFacts = append(allFacts, emitFacts(pf, table)...)
	}
	return allFacts, nil
}

func main() {
	showVersion := flag.Bool("version", false, "Print adapter version")
	showV := flag.Bool("v", false, "Print adapter version")
	flag.Parse()

	if *showVersion || *showV {
		fmt.Println(AdapterVersion)
		os.Exit(0)
	}

	args := flag.Args()
	if len(args) < 1 {
		fmt.Println("Usage: lena-adapter-go <target_directory>")
		os.Exit(1)
	}
	targetDir := args[0]
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
