#ifndef PLAYGROUND_CUDA_ORDER_CUH
#define PLAYGROUND_CUDA_ORDER_CUH

#include <cstddef>

struct OrderItem {
  const char *name;
  int quantity;
  double price;

  __host__ __device__ double total() const;
};

struct Order {
  const char *customer;
  const OrderItem *items;
  std::size_t item_count;

  __host__ __device__ double total() const;
};

Order sample_order();

#endif
