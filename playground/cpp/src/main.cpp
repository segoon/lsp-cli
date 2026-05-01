#include <iostream>

#include "order.hpp"

int main() {
    playground::Order order("Grace");
    order.add_item({"Notebook", 3, 4.5});
    order.add_item({"Pen", 5, 1.2});

    std::cout << playground::report::format_order(order) << '\n';
    return 0;
}
