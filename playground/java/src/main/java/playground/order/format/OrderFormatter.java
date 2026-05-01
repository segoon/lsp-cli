package playground.order.format;

import playground.order.Order;

public final class OrderFormatter {
    public String format(Order order) {
        return "%s has %d items worth %.2f".formatted(
            order.customer(),
            order.items().size(),
            order.total()
        );
    }
}
