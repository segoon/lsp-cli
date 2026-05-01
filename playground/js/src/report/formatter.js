export function formatOrder(order) {
  return `${order.customer} has ${order.items.length} items worth ${order.total().toFixed(2)}`;
}
