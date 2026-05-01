from app.models import Order


def format_order(order: Order) -> str:
    return (
        f"{order.customer} has {len(order.items)} items "
        f"worth {order.total():.2f}"
    )
