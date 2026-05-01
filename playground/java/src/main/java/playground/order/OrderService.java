package playground.order;

import java.util.List;

public final class OrderService {
    public Order createSampleOrder() {
        return new Order(
            "Linus",
            List.of(
                new OrderItem("SSD", 1, 89.0),
                new OrderItem("Bracket", 2, 6.5)
            )
        );
    }
}
