#ifndef PLAYGROUND_OBJC_ORDER_H
#define PLAYGROUND_OBJC_ORDER_H

#include <stddef.h>

typedef struct {
  const char *name;
  int quantity;
  double price;
} OrderItem;

__attribute__((objc_root_class))
@interface Order {
@private
  const char *_customer;
  const OrderItem *_items;
  size_t _itemCount;
}

- (instancetype)initWithCustomer:(const char *)customer
                           items:(const OrderItem *)items
                           count:(size_t)itemCount;
- (const char *)customer;
- (size_t)itemCount;
- (double)total;

@end

double item_total(OrderItem item);
Order *sample_order(void);

#endif
