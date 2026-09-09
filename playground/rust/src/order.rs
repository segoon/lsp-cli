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

fn new_item(name: &str, quantity: u32, price: f64) -> OrderItem {
    OrderItem {
        name: name.to_string(),
        quantity,
        price,
    }
}

pub fn sample_order() -> Order {
    Order {
        customer: "Carol".to_string(),
        items: vec![new_item("Mouse", 1, 35.0), new_item("Pad", 1, 12.5)],
    }
}
