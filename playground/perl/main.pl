use strict;
use warnings;
use lib 'lib';
use Order;
use OrderItem;
use Report qw(format_order);

sub build_sample_order {
    my @items = (
        OrderItem->new("Mouse", 1, 35.0),
        OrderItem->new("Pad",   1, 12.5),
    );
    return Order->new("Carol", \@items);
}

my $order = build_sample_order();
print format_order($order), "\n";
