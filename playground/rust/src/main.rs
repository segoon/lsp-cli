mod order;
mod report;

fn main() {
    let order = order::sample_order();
    println!("{}", report::format_order(&order));
}
