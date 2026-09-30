#include <sstream>

#include "order.hpp"

namespace playground::report {

std::string format_order(const Order &order) {
    std::ostringstream output;
    output << order.customer() << " has " << order.items().size() << " items worth "
           << order.total();
    return output.str();
}

std::string PlainOrderFormatter::format(const Order &order) const {
    return format_order(order);
}

}  // namespace playground::report
