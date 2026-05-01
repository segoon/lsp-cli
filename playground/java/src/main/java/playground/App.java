package playground;

import playground.order.Order;
import playground.order.OrderService;
import playground.order.format.OrderFormatter;

public final class App {
    public static void main(String[] args) {
        OrderService service = new OrderService();
        Order order = service.createSampleOrder();
        OrderFormatter formatter = new OrderFormatter();

        System.out.println(formatter.format(order));
    }
}
