package main

import (
	"fmt"
	"os"
	"runtime"
)

func main() {
	fmt.Println("Hello from Go in Singleload!")
	fmt.Printf("Go version: %s\n", runtime.Version())
	fmt.Printf("OS/Arch: %s/%s\n", runtime.GOOS, runtime.GOARCH)
	fmt.Printf("User: %s\n", os.Getenv("USER"))
}
