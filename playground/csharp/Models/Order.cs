namespace Playground.Models;

public sealed class Order
{
    public required string Customer { get; init; }

    public required IReadOnlyList<OrderItem> Items { get; init; }

    public decimal Total() => Items.Sum(item => item.Total());
}
