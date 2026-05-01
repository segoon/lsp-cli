using Playground.Models;

namespace Playground.Services;

public sealed class OrderService
{
    public Order BuildSampleOrder() => new()
    {
        Customer = "Anders",
        Items =
        [
            new OrderItem { Name = "Monitor", Quantity = 1, Price = 250m },
            new OrderItem { Name = "Arm", Quantity = 1, Price = 45m },
        ],
    };
}
