#include "order.hpp"

namespace playground {

double OrderItem::total() const {
    return price * quantity;
}

Order::Order(std::string customer) : customer_(std::move(customer)) {}

void Order::add_item(OrderItem item) {
    items_.push_back(std::move(item));
}

double Order::total() const {
    double value = 0.0;

    for (const OrderItem &item : items_) {
        value += item.total();
    }

    return value;
}

const std::string &Order::customer() const {
    return customer_;
}

const std::vector<OrderItem> &Order::items() const {
    return items_;
}

}  // namespace playground
