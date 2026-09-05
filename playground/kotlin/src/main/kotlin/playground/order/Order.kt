package playground.order

interface OrderTotal {
    fun total(): Double
}

data class OrderItem(
    val name: String,
    val quantity: Int,
    val price: Double
) : OrderTotal {
    override fun total(): Double = quantity * price
}

data class Order(
    val customer: String,
    val items: List<OrderItem>
) : OrderTotal {
    override fun total(): Double = items.map { item -> item.total() }.sum()
}
