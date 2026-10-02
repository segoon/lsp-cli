#ifndef PLAYGROUND_CUDA_REPORT_CUH
#define PLAYGROUND_CUDA_REPORT_CUH

#include <string>

#include "order.cuh"

std::string format_order(const Order &order);

class OrderFormatting {
public:
  virtual ~OrderFormatting() = default;
  virtual std::string format(const Order &order) const = 0;
};

class PlainOrderFormatter final : public OrderFormatting {
public:
  std::string format(const Order &order) const override;
};

#endif
