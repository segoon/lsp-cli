const std = @import("std");

const OrderItem = struct {
    name: []const u8,
    quantity: u32,
    price: f64,

    fn total(self: OrderItem) f64 {
        return @as(f64, @floatFromInt(self.quantity)) * self.price;
    }
};

const Order = struct {
    customer: []const u8,
    items: []const OrderItem,

    fn total(self: Order) f64 {
        var sum: f64 = 0;
        for (self.items) |item| {
            sum += item.total();
        }
        return sum;
    }
};

fn sample_order() Order {
    return Order{
        .customer = "Carol",
        .items = &[_]OrderItem{
            OrderItem{ .name = "Mouse", .quantity = 1, .price = 35.0 },
            OrderItem{ .name = "Pad", .quantity = 1, .price = 12.5 },
        },
    };
}

fn format_order(order: Order, buffer: []u8) ![]u8 {
    return std.fmt.bufPrint(buffer, "{s} has {d} items worth {d:.2}", .{
        order.customer,
        order.items.len,
        order.total(),
    });
}

pub fn main() !void {
    const order = sample_order();
    var buffer: [128]u8 = undefined;
    const report = try format_order(order, &buffer);
    std.debug.print("{s}\n", .{report});
}
