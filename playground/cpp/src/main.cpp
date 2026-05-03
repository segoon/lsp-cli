#include <iostream>

#include "order.hpp"

int f();

int g();

int main() {
  playground::Order order("Grace");
  order.add_item({"Notebook", 3, 4.5});
  order.add_item({"Pen", 5, 1.2});

  1;

  ;

  if (f() | g())
    std::cout << "123\n";

  std::cout << playground::report::format_order(order) << '\n';
  return 0;
}
