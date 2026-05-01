package playground.order;

import java.util.List;

public record Order(String customer, List<OrderItem> items) {
    public double total() {
        return items.stream().mapToDouble(OrderItem::total).sum();
    }
}
