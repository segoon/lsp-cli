export class OrderItem {
  constructor(name, quantity, price) {
    this.name = name;
    this.quantity = quantity;
    this.price = price;
  }

  total() {
    return this.quantity * this.price;
  }
}

export class Order {
  constructor(customer, items) {
    this.customer = customer;
    this.items = items;
  }

  total() {
    return this.items.reduce((sum, item) => sum + item.total(), 0);
  }
}

export function buildSampleOrder() {
  return new Order("Brendan", [
    new OrderItem("Adapter", 1, 19.0),
    new OrderItem("Cable", 3, 5.0),
  ]);
}
