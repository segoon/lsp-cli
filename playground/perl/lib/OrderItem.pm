package OrderItem;

use strict;
use warnings;

sub new {
    my ($class, $name, $quantity, $price) = @_;
    return bless { name => $name, quantity => $quantity, price => $price }, $class;
}

1;
