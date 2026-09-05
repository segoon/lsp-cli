#include "OrderFormatter.h"

#include <stdio.h>

void format_order(Order *order, char *buffer, size_t buffer_size) {
  if (order == (Order *)0) {
    snprintf(buffer, buffer_size, "empty order");
    return;
  }
  snprintf(buffer, buffer_size, "%s has %zu items worth %.2f", [order customer],
           [order itemCount], [order total]);
}
