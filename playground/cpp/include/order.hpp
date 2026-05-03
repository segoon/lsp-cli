#ifndef PLAYGROUND_CPP_ORDER_HPP
#define PLAYGROUND_CPP_ORDER_HPP

#include <string>
#include <vector>

namespace playground {

struct OrderItem {
  std::string name;
  int quantity;
  double price;

  double total() const;
};

class Order {
public:
  explicit Order(std::string customer);

  void add_item(OrderItem item);
  double total() const;
  const std::string &customer() const;
  const std::vector<OrderItem> &items() const;

private:
  std::string customer_;
  std::vector<OrderItem> items_;
};

int f(int arg);

namespace report {
std::string format_order(const Order &order);
}

} // namespace playground

#endif
