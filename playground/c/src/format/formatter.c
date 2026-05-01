#include <stdio.h>

#include "order.h"

void format_order(const Order *order, char *buffer, size_t buffer_size) {
    snprintf(
        buffer,
        buffer_size,
        "%s has %zu items worth %.2f",
        order->customer,
        order->item_count,
        order_total(order)
    );
}
