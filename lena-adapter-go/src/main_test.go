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
		// Module FQSN should be go:edgecases, no file extensions!
		if strings.Contains(f, ":type/module") && strings.Contains(f, "go:edgecases") {
			moduleFound = true
			break
		}
	}
	if !moduleFound {
		t.Error("Expected logical module entity for edgecases")
	}
}

func TestGoDecomposedAttributes(t *testing.T) {
	fset := token.NewFileSet()
	facts, _ := RunTwoPass(fset, []string{getFixturePath("edge_cases.go")})

	output := strings.Join(facts, "\n")

	if !strings.Contains(output, ":symbol/container \"MyStruct\"") {
		t.Error("Failed to extract container struct name")
	}
	if !strings.Contains(output, ":symbol/module \"edgecases\"") {
		t.Error("Failed to extract module name")
	}
	if !strings.Contains(output, ":symbol/name \"PointerReceiver\"") {
		t.Error("Failed to extract atomic function name")
	}
	if !strings.Contains(output, ":lang/go") {
		t.Error("Missing standardized :lang/go language tag")
	}
}

func TestGoExternalStubEmission(t *testing.T) {
	fset := token.NewFileSet()
	facts, _ := RunTwoPass(fset, []string{getFixturePath("edge_cases.go")})

	output := strings.Join(facts, "\n")

	// Check for fmt.Println external stub
	if !strings.Contains(output, ":symbol/external true") {
		t.Error("Failed to emit external stubs")
	}
	if !strings.Contains(output, ":symbol/fqsn \"go:fmt/Println\"") {
		t.Error("Missing specific external stub FQSN")
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

func TestGoAdapterVersion(t *testing.T) {
	if AdapterVersion != "0.2.0" {
		t.Errorf("Expected AdapterVersion 0.2.0, got %s", AdapterVersion)
	}
}

func TestGoModuleNoExtension(t *testing.T) {
	fset := token.NewFileSet()
	facts, _ := RunTwoPass(fset, []string{getFixturePath("edge_cases.go")})

	for _, f := range facts {
		if strings.Contains(f, ":symbol/module") && strings.Contains(f, ".go") {
			t.Errorf("Found file extension in :symbol/module: %s", f)
		}
	}
}
