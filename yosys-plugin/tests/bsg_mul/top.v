`include "bsg_mul.v"
`include "bsg_mul_pipelined.v"

module top(
    input    [6-1:0]   x_i
    , input  [6-1:0]   y_i
    , input signed_i
    , output [6*2-1:0] z_o
);

 bsg_mul  #( .width_p(6) ) bsg_mul (
        .x_i(x_i),
        .y_i(y_i),
        .signed_i(signed_i),
        .z_o(z_o)
 );

 // need to do some stuff - mayb

endmodule
