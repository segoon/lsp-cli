package playground.report

import playground.order.Order

fun formatOrder(order: Order): String =
    "${order.customer} has ${order.items.size} items worth ${"%.2f".format(order.total())}"
