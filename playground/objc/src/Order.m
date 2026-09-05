#include "Order.h"

@implementation Order

- (instancetype)initWithCustomer:(const char *)customer
                           items:(const OrderItem *)items
                           count:(size_t)itemCount {
  _customer = customer;
  _items = items;
  _itemCount = itemCount;
  return self;
}

- (const char *)customer {
  return _customer;
}

- (size_t)itemCount {
  return _itemCount;
}

- (double)total {
  double value = 0.0;
  for (size_t index = 0; index < _itemCount; ++index) {
    value += item_total(_items[index]);
  }
  return value;
}

@end

double item_total(OrderItem item) { return item.quantity * item.price; }

Order *sample_order(void) {
  static const OrderItem items[] = {
      {"Display", 1, 600.0},
      {"Stand", 1, 80.0},
  };
  Order *order = (Order *)0;
  return [order initWithCustomer:"Brad" items:items count:2];
}
