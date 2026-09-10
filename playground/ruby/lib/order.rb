class OrderItem
  attr_reader :name, :quantity, :price

  def initialize(name, quantity, price)
    @name = name
    @quantity = quantity
    @price = price
  end

  def total
    quantity * price
  end
end

class Order
  attr_reader :customer, :items

  def initialize(customer, items)
    @customer = customer
    @items = items
  end

  def total
    items.sum(&:total)
  end
end

def build_sample_order
  Order.new(
    "Carol",
    [
      OrderItem.new("Mouse", 1, 35.0),
      OrderItem.new("Pad", 1, 12.5)
    ]
  )
end
