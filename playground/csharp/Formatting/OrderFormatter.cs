using Playground.Models;

namespace Playground.Formatting;

public sealed class OrderFormatter
{
    public string Format(Order order) =>
        $"{order.Customer} has {order.Items.Count} items worth {order.Total():0.00}";
}
