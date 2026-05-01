#include "order.h"

double item_total(const OrderItem *item) {
    return item->price * item->quantity;
}

double order_total(const Order *order) {
    double total = 0.0;

    for (size_t index = 0; index < order->item_count; index++) {
        total += item_total(&order->items[index]);
    }

    return total;
}
