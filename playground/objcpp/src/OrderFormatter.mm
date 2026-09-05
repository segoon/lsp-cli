#include "OrderFormatter.hpp"

#include <sstream>

std::string format_order(Order *order) {
  if (order == nullptr) {
    return "empty order";
  }
  std::ostringstream output;
  output << [order customer] << " has " << [order items].size()
         << " items worth " << [order total];
  return output.str();
}
