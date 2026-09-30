use crate::order::{Order, OrderTotaling};

pub fn format_order(order: &Order) -> String {
    format!(
        "{} has {} items worth {:.2}",
        order.customer,
        order.items.len(),
        order.total()
    )
}
