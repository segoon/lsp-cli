from app.models import Order, OrderItem


def build_sample_order() -> Order:
    return Order(
        customer="Matz",
        items=[
            OrderItem(name="Book", quantity=2, price=12.0),
            OrderItem(name="Sticker", quantity=4, price=1.5),
        ],
    )
