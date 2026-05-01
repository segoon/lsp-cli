#include <stdio.h>

#include "order.h"

int main(void) {
    OrderItem items[] = {
        {"Keyboard", 1, 99.0},
        {"Cable", 2, 7.5},
    };
    Order order = {"Ada", items, 2};
    char summary[128];

    format_order(&order, summary, sizeof(summary));
    puts(summary);

    return 0;
}
