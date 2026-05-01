package playground.order;

public record OrderItem(String name, int quantity, double price) {
    public double total() {
        return quantity * price;
    }
}
