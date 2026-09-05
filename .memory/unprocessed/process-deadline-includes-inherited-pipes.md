# Process deadline must include inherited output pipes

Waiting with a deadline only on the process-group leader is insufficient. The leader can exit while
a descendant remains alive with inherited stdout or stderr descriptors, causing output-reader joins
to block forever after the nominal deadline. Apply the same absolute deadline to both leader exit
and pipe closure; if either pipe remains open, kill the still-addressable process group before
joining readers.
