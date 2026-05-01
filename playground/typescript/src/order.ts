export class OrderItem {
  constructor(
    public readonly name: string,
    public readonly quantity: number,
    public readonly price: number,
  ) {}

  total(): number {
    return this.quantity * this.price;
  }
}

export class Order {
  constructor(
    public readonly customer: string,
    public readonly items: OrderItem[],
  ) {}

  total(): number {
    return this.items.reduce((sum, item) => sum + item.total(), 0);
  }
}

export function buildSampleOrder(): Order {
  return new Order("Daniel", [
    new OrderItem("Desk", 1, 199),
    new OrderItem("Lamp", 1, 39),
  ]);
}
