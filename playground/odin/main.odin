package main

import "core:fmt"

OrderItem :: struct {
	name:     string,
	quantity: int,
	price:    f64,
}

Order :: struct {
	customer: string,
	items:    []OrderItem,
}

order_total :: proc(order: Order) -> f64 {
	sum := 0.0
	for item in order.items {
		sum += f64(item.quantity) * item.price
	}
	return sum
}

build_sample_order :: proc() -> Order {
	return Order{
		customer = "Carol",
		items = []OrderItem{
			{name = "Mouse", quantity = 1, price = 35.0},
			{name = "Pad", quantity = 1, price = 12.5},
		},
	}
}

format_order :: proc(order: Order) -> string {
	total := order_total(order)
	return fmt.tprintf("%s has %d items worth %.2f", order.customer, len(order.items), total)
}

main :: proc() {
	order := build_sample_order()
	fmt.println(format_order(order))
}
