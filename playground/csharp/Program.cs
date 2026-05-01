using Playground.Formatting;
using Playground.Services;

var service = new OrderService();
var order = service.BuildSampleOrder();
var formatter = new OrderFormatter();

Console.WriteLine(formatter.Format(order));
