#include "Order.hpp"
#include "OrderFormatter.hpp"

#include <iostream>

int main() {
  Order *order = sample_order();
  std::cout << format_order(order) << '\n';
  return 0;
}
