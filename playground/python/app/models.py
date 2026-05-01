from dataclasses import dataclass


@dataclass(slots=True)
class OrderItem:
    name: str
    quantity: int
    price: float

    def total(self) -> float:
        return self.quantity * self.price


@dataclass(slots=True)
class Order:
    customer: str
    items: list[OrderItem]

    def total(self) -> float:
        return sum(item.total() for item in self.items)
