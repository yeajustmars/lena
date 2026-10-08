// astrolabe-adapter-go/tests/fixtures/go/edge_cases.go

package edgecases

import "fmt"

// StandaloneFunc is a standard function with a docstring.
func StandaloneFunc() {
	fmt.Println("Hello")
}

type MyStruct struct {
	ID int
}

// PointerReceiver method
func (m *MyStruct) PointerReceiver() {
	fmt.Println(m.ID)
}

// ValueReceiver method
func (m MyStruct) ValueReceiver() {
	fmt.Println(m.ID)
}
