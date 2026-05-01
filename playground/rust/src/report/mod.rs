use crate::order::Order;

pub fn format_order(order: &Order) -> String {
    format!(
        "{} has {} items worth {:.2}",
        order.customer,
        order.items.len(),
        order.total()
    )
}
