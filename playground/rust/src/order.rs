pub struct OrderItem {
    pub name: String,
    pub quantity: u32,
    pub price: f64,
}

impl OrderItem {
    pub fn total(&self) -> f64 {
        self.quantity as f64 * self.price
    }
}

pub struct Order {
    pub customer: String,
    pub items: Vec<OrderItem>,
}

impl Order {
    pub fn total(&self) -> f64 {
        self.items.iter().map(OrderItem::total).sum()
    }
}

pub fn sample_order() -> Order {
    Order {
        customer: "Carol".to_string(),
        items: vec![
            OrderItem {
                name: "Mouse".to_string(),
                quantity: 1,
                price: 35.0,
            },
            OrderItem {
                name: "Pad".to_string(),
                quantity: 1,
                price: 12.5,
            },
        ],
    }
}
