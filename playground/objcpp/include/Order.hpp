#ifndef PLAYGROUND_OBJCPP_ORDER_HPP
#define PLAYGROUND_OBJCPP_ORDER_HPP

#include <string>
#include <vector>

struct OrderItem {
  std::string name;
  int quantity;
  double price;

  double total() const;
};

@protocol OrderTotaling
- (double)total;
@end

__attribute__((objc_root_class))
@interface Order<OrderTotaling> {
@private
  std::string _customer;
  std::vector<OrderItem> _items;
}

- (instancetype)initWithCustomer:(std::string)customer
                           items:(std::vector<OrderItem>)items;
- (const std::string &)customer;
- (const std::vector<OrderItem> &)items;
- (double)total;

@end

Order *sample_order();

#endif
