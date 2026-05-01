#ifndef PLAYGROUND_C_ORDER_H
#define PLAYGROUND_C_ORDER_H

#include <stddef.h>

typedef struct {
    const char *name;
    int quantity;
    double price;
} OrderItem;

typedef struct {
    const char *customer;
    const OrderItem *items;
    size_t item_count;
} Order;

double item_total(const OrderItem *item);
double order_total(const Order *order);
void format_order(const Order *order, char *buffer, size_t buffer_size);

#endif
