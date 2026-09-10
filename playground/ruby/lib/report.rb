def format_order(order)
  "#{order.customer} has #{order.items.length} items worth #{format('%.2f', order.total)}"
end
