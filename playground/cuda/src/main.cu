#include "order.cuh"
#include "report.cuh"

#include <iostream>

__global__ void calculate_order_total(Order order, double *result) {
  *result = order.total();
}

int main() {
  const Order order = sample_order();
  std::cout << format_order(order) << '\n';
  return 0;
}
