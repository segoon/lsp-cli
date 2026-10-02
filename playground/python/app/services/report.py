from abc import ABC, abstractmethod

from app.models import Order


class OrderFormatting(ABC):
    @abstractmethod
    def format(self, order: Order) -> str: ...


class PlainOrderFormatter(OrderFormatting):
    def format(self, order: Order) -> str:
        return (
            f"{order.customer} has {len(order.items)} items "
            f"worth {order.total():.2f}"
        )


def format_order(order: Order) -> str:
    return PlainOrderFormatter().format(order)
