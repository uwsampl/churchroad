// RUN: cargo run -- \
// RUN:   --filepath %s \
// RUN:   --top-module-name mul \
// RUN:   --architecture xilinx-ultrascale-plus \
// RUN:   --simulate \
// TODO Hardcoded
// RUN:   --simulate-with-verilator-arg="--verilator_include_dir=/Users/gus/lakeroad-private/DSP48E2" \
// RUN:   --simulate-with-verilator-arg="--verilator_extra_arg=-DXIL_XECLIB" \
// RUN:   --simulate-with-verilator-arg="--verilator_extra_arg=-Wno-UNOPTFLAT" \
// RUN:   --simulate-with-verilator-arg="--verilator_extra_arg=-Wno-COMBDLY" \
// RUN:   --simulate-with-verilator-arg="--verilator_extra_arg=-Wno-LATCH" \
// RUN:   --simulate-with-verilator-arg="--verilator_extra_arg=-Wno-WIDTH" \
// RUN:   --simulate-with-verilator-arg="--verilator_extra_arg=-Wno-STMTDLY" \
// RUN:   --simulate-with-verilator-arg="--verilator_extra_arg=-Wno-CASEX" \
// RUN:   --simulate-with-verilator-arg="--verilator_extra_arg=-Wno-TIMESCALEMOD" \
// RUN:   --simulate-with-verilator-arg="--verilator_extra_arg=-Wno-PINMISSING" \
// RUN: | FileCheck %s

module mul(input [15:0] a, b, output [15:0] out);
  assign out = a * b;
endmodule

// TODO wrong module name
// CHECK: module top(
// CHECK:   input [16-1:0] a,
// CHECK:   input [16-1:0] b,
// CHECK:   output [16-1:0] out,
// CHECK: );
// CHECK:   DSP48E2 #(