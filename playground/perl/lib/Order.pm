package Order;

use strict;
use warnings;

sub new {
    my ($class, $customer, $items) = @_;
    return bless { customer => $customer, items => $items }, $class;
}

sub total {
    my ($self) = @_;
    my $sum = 0;
    $sum += $_->{quantity} * $_->{price} for @{ $self->{items} };
    return $sum;
}

1;
