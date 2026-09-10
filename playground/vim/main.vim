function! OrderTotal(quantity, price)
    return a:quantity * a:price
endfunction

function! BuildSampleOrder()
    return "Carol"
endfunction

function! FormatOrder(customer, quantity, price)
    let total = OrderTotal(a:quantity, a:price)
    return a:customer . " has " . a:quantity . " items worth " . total
endfunction

echo FormatOrder(BuildSampleOrder(), 2, 35.0)
