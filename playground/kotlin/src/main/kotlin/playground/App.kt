package playground

import playground.order.sampleOrder
import playground.report.formatOrder

fun main() {
    val order = sampleOrder()
    println(formatOrder(order))
}
