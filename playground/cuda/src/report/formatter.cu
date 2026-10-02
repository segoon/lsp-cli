#include "report.cuh"

#include <sstream>

std::string format_order(const Order &order) {
  std::ostringstream output;
  output << order.customer << " has " << order.item_count << " items worth "
         << order.total();
  return output.str();
}

std::string PlainOrderFormatter::format(const Order &order) const {
  return format_order(order);
}
