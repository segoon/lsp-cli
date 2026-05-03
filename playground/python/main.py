from app.services.orders import build_sample_order
from app.services.report import format_order


def main() -> None:
    order = build_sample_order()
    print(format_order(order))



if __name__ == "__main__":
    main()
