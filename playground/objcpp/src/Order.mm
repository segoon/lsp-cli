#include "Order.hpp"

#include <utility>

double OrderItem::total() const {
  return static_cast<double>(quantity) * price;
}

@implementation Order

- (instancetype)initWithCustomer:(std::string)customer
                           items:(std::vector<OrderItem>)items {
  _customer = std::move(customer);
  _items = std::move(items);
  return self;
}

- (const std::string &)customer {
  return _customer;
}

- (const std::vector<OrderItem> &)items {
  return _items;
}

- (double)total {
  double value = 0.0;
  for (const OrderItem &item : _items) {
    value += item.total();
  }
  return value;
}

@end

Order *sample_order() {
  Order *order = (Order *)nullptr;
  std::vector<OrderItem> items = {
      {"Compiler", 1, 75.0},
      {"Book", 2, 42.0},
  };
  return [order initWithCustomer:std::string("Bjarne") items:std::move(items)];
}
