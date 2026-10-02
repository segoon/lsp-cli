type OrderItem = {
    name: string,
    quantity: number,
    price: number,
}

type Order = {
    customer: string,
    items: { OrderItem },
}

local function order_total(order: Order): number
    local sum = 0
    for _, item in order.items do
        sum += item.quantity * item.price
    end
    return sum
end

local function sample_order_items(): { OrderItem }
    return {
        { name = "Mouse", quantity = 1, price = 35.0 },
        { name = "Pad", quantity = 1, price = 12.5 },
    }
end

local function build_sample_order(): Order
    return {
        customer = "Carol",
        items = sample_order_items(),
    }
end

local function format_order(order: Order): string
    return string.format("%s has %d items worth %.2f", order.customer, #order.items, order_total(order))
end

local function build_order_report(): string
    return format_order(build_sample_order())
end

print(build_order_report())
