import { Order } from "../order";

export function formatOrder(order: Order): string {
  return `${order.customer} has ${order.items.length} items worth ${order.total().toFixed(2)}`;
}
