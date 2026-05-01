package report

import (
    "fmt"

    "example.com/lsp-cli-playground-go/internal/order"
)

func Format(value order.Order) string {
    return fmt.Sprintf(
        "%s has %d items worth %.2f",
        value.Customer,
        len(value.Items),
        value.Total(),
    )
}
