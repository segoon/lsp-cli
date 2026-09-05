#include "order.cuh"

__host__ __device__ double OrderItem::total() const {
  return static_cast<double>(quantity) * price;
}

__host__ __device__ double Order::total() const {
  double value = 0.0;
  for (std::size_t index = 0; index < item_count; ++index) {
    value += items[index].total();
  }
  return value;
}

Order sample_order() {
  static const OrderItem items[] = {
      {"GPU", 1, 499.0},
      {"Power Cable", 2, 12.5},
  };
  return {"Jensen", items, 2};
}
