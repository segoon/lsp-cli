mod order;
mod report;

fn main() {
    let order = order::sample_order();
    let _typed_fixture = &order::SAMPLE_ORDER_VALUE;
    println!("{}", report::format_order(&order));
}
