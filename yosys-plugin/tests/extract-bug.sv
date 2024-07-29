// This test tests Verilog that used to produce buggy Churchroad:
// ...
// (let v2 (Op1 (Extract 0 0) v0))
// (union v1 (Op1 (Extract 1 1) v2))
// ...
//
// RUN: $YOSYS -q -m $CHURCHROAD_DIR/yosys-plugin/churchroad.so \
// RUN:   -p 'read_verilog -sv %s; prep -top test; write_churchroad' \
// RUN:   | FileCheck %s

module test(input [1:0] in, output out);
  assign out = in[1];
endmodule

// CHECK: (let v0 (Wire "v0" 2))
// CHECK: (let v1 (Wire "v1" 1))
// CHECK: (union v1 (Op1 (Extract 1 1) v0))
// CHECK: (let in (Var "in" 2))
// CHECK: (IsPort "" "in" (Input) in)
// CHECK: (union v0 in)
// CHECK: (let out v1)
// CHECK: (IsPort "" "out" (Output) out)
// CHECK: (delete (Wire "v0" 2))
// CHECK: (delete (Wire "v1" 1))
