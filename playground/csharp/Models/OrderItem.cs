namespace Playground.Models;

public sealed class OrderItem
{
    public required string Name { get; init; }

    public required int Quantity { get; init; }

    public required decimal Price { get; init; }

    public decimal Total() => Quantity * Price;
}
