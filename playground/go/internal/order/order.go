package order

type Item struct {
    Name     string
    Quantity int
    Price    float64
}

func (item Item) Total() float64 {
    return float64(item.Quantity) * item.Price
}

type Order struct {
    Customer string
    Items    []Item
}

func (order Order) Total() float64 {
    total := 0.0
    for _, item := range order.Items {
        total += item.Total()
    }
    return total
}

func SampleOrder() Order {
    return Order{
        Customer: "Ken",
        Items: []Item{
            {Name: "Router", Quantity: 1, Price: 79.0},
            {Name: "Patch Cable", Quantity: 2, Price: 4.0},
        },
    }
}
