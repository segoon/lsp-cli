package playground.order

fun sampleOrder(): Order = Order(
    customer = "JetBrains",
    items = listOf(
        OrderItem(name = "Keyboard", quantity = 1, price = 120.0),
        OrderItem(name = "Keycap", quantity = 4, price = 3.5)
    )
)
