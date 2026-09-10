package Report;

use strict;
use warnings;
use Exporter qw(import);

our @EXPORT_OK = qw(format_order);

sub format_order {
    my ($order) = @_;
    my $count = scalar @{ $order->{items} };
    return sprintf(
        "%s has %d items worth %.2f",
        $order->{customer}, $count, $order->total
    );
}

1;
