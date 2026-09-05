#include "Order.h"
#include "OrderFormatter.h"

#include <stdio.h>

int main(void) {
  Order *order = sample_order();
  char summary[128];
  format_order(order, summary, sizeof(summary));
  puts(summary);
  return 0;
}
