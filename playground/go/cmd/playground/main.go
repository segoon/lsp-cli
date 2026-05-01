package main

import (
    "fmt"

    "example.com/lsp-cli-playground-go/internal/order"
    "example.com/lsp-cli-playground-go/internal/report"
)

func main() {
    sample := order.SampleOrder()
    fmt.Println(report.Format(sample))
}
