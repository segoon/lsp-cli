#include "OrderFormatter.hpp"

#include <sstream>

class OrderFormatting {
public:
  virtual ~OrderFormatting() = default;
  virtual std::string format(Order *order) const = 0;
};

class PlainOrderFormatter final : public OrderFormatting {
public:
  std::string format(Order *order) const override;
};

std::string format_order(Order *order) {
  if (order == nullptr) {
    return "empty order";
  }
  std::ostringstream output;
  output << [order customer] << " has " << [order items].size()
         << " items worth " << [order total];
  return output.str();
}

std::string PlainOrderFormatter::format(Order *order) const {
  return format_order(order);
}
