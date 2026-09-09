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

func newItem(name string, quantity int, price float64) Item {
    return Item{Name: name, Quantity: quantity, Price: price}
}

func SampleOrder() Order {
    return Order{
        Customer: "Ken",
        Items: []Item{
            newItem("Router", 1, 79.0),
            newItem("Patch Cable", 2, 4.0),
        },
    }
}
