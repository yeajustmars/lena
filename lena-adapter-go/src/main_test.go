// lena-adapter-go/src/main_test.go

package main

import (
	"go/token"
	"os"
	"path/filepath"
	"strconv"
	"strings"
	"testing"
)

func getFixturePath(file string) string {
	cwd, _ := os.Getwd()
	return filepath.Join(cwd, "..", "tests", "fixtures", "go", file)
}

func countDbAdds(facts []string) int {
	count := 0
	for _, f := range facts {
		count += strings.Count(f, "[:db/add")
	}
	return count
}

func TestGoEdgeCasesParse(t *testing.T) {
	fset := token.NewFileSet()
	facts, err := RunTwoPass(fset, []string{getFixturePath("edge_cases.go")})
	if err != nil {
		t.Fatalf("Failed to extract facts: %v", err)
	}

	if len(facts) == 0 {
		t.Fatal("Expected facts to be extracted, got 0")
	}
}

func TestGoModuleEntityCreation(t *testing.T) {
	fset := token.NewFileSet()
	facts, _ := RunTwoPass(fset, []string{getFixturePath("edge_cases.go")})

	moduleFound := false
	for _, f := range facts {
		if strings.Contains(f, ":type/module") && strings.Contains(f, "edge_cases.go") {
			moduleFound = true
			break
		}
	}
	if !moduleFound {
		t.Error("Expected module entity for edge_cases.go")
	}
}

func TestGoMethodReceiverCorrelation(t *testing.T) {
	fset := token.NewFileSet()
	facts, _ := RunTwoPass(fset, []string{getFixturePath("edge_cases.go")})

	output := strings.Join(facts, "\n")
	if !strings.Contains(output, "\"go:edgecases.MyStruct/PointerReceiver\"") {
		t.Error("Failed to correctly correlate pointer receiver method")
	}
	if !strings.Contains(output, "\"go:edgecases.MyStruct/ValueReceiver\"") {
		t.Error("Failed to correctly correlate value receiver method")
	}
}

func TestGoDocstringExtraction(t *testing.T) {
	fset := token.NewFileSet()
	facts, _ := RunTwoPass(fset, []string{getFixturePath("edge_cases.go")})

	output := strings.Join(facts, "\n")
	if !strings.Contains(output, ":doc/text \"StandaloneFunc is a standard function") {
		t.Error("Failed to extract docstring")
	}
}

func TestGoHashIdMasking(t *testing.T) {
	fset := token.NewFileSet()
	facts, _ := RunTwoPass(fset, []string{getFixturePath("edge_cases.go")})

	for _, f := range facts {
		parts := strings.Split(f, " ")
		for i, part := range parts {
			if part == "[:db/add" && i+1 < len(parts) {
				idStr := parts[i+1]
				id, err := strconv.ParseUint(idStr, 10, 64)
				if err == nil {
					if id > 0x7FFFFFFFFFFFFFFF {
						t.Errorf("Hash ID %d exceeds positive 64-bit threshold", id)
					}
				}
			}
		}
	}
}

func TestGoEmptyFileGracefulHandling(t *testing.T) {
	emptyPath := getFixturePath("empty.go")
	os.WriteFile(emptyPath, []byte(""), 0644)
	defer os.Remove(emptyPath)

	fset := token.NewFileSet()
	facts, err := RunTwoPass(fset, []string{emptyPath})

	// go/parser throws an error on completely empty files (expected package declaration)
	if err == nil {
		t.Error("Expected parsing error for empty file, got nil")
	}
	if len(facts) != 0 {
		t.Error("Expected 0 facts from empty file")
	}
}

func TestGoMalformedSyntaxResilience(t *testing.T) {
	malformedPath := getFixturePath("malformed.go")
	os.WriteFile(malformedPath, []byte("package bad\nfunc A() {\n"), 0644)
	defer os.Remove(malformedPath)

	fset := token.NewFileSet()
	_, err := RunTwoPass(fset, []string{malformedPath})

	if err == nil {
		t.Error("Expected parser to surface syntax error")
	}
}

func TestGoFunctionPositions(t *testing.T) {
	fset := token.NewFileSet()
	facts, _ := RunTwoPass(fset, []string{getFixturePath("edge_cases.go")})

	output := strings.Join(facts, "\n")
	if !strings.Contains(output, ":location/start-line") || !strings.Contains(output, ":location/start-col") {
		t.Error("Missing physical line/col metadata")
	}
}

func TestGoMultipleFunctions(t *testing.T) {
	fset := token.NewFileSet()
	facts, _ := RunTwoPass(fset, []string{getFixturePath("edge_cases.go")})

	fnCount := 0
	for _, f := range facts {
		if strings.Contains(f, ":type/function") {
			fnCount++
		}
	}
	if fnCount != 3 {
		t.Errorf("Expected 3 functions, found %d", fnCount)
	}
}

func TestGoGinStressAddCount(t *testing.T) {
	ginPath := getFixturePath("gin/gin/1.9.1/gin.go")
	if _, err := os.Stat(ginPath); os.IsNotExist(err) {
		t.Skip("Gin fixture not found, skipping stress test. Run download script first.")
	}

	fset := token.NewFileSet()
	facts, err := RunTwoPass(fset, []string{ginPath})
	if err != nil {
		t.Fatalf("Failed to parse Gin: %v", err)
	}

	addCount := countDbAdds(facts)
	if addCount <= 100 {
		t.Errorf("Expected >100 [:db/add assertions for Gin, got %d", addCount)
	}
}

func TestGoCallSiteExtraction(t *testing.T) {
	fset := token.NewFileSet()
	facts, _ := RunTwoPass(fset, []string{getFixturePath("edge_cases.go")})

	output := strings.Join(facts, "\n")

	// Did it create call-site entities?
	if !strings.Contains(output, ":type/call-site") {
		t.Error("Failed to extract any :type/call-site entities")
	}

	// Did it catch the fmt.Println call inside StandaloneFunc?
	// The callee ID should map to "go:fmt/Println" because localImports maps "fmt" to "fmt".
	if !strings.Contains(output, ":call/caller") || !strings.Contains(output, ":call/callee") {
		t.Error("Missing :call/caller or :call/callee edges on call-site")
	}
}
