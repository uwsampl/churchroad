// RUN: $YOSYS -q -m $CHURCHROAD_DIR/yosys-plugin/churchroad.so \
// RUN:   -p 'read_verilog -sv %s; prep -top test; pmuxtree; prep; write_churchroad' \
// RUN:   | FileCheck %s

module test(input a, b, output out);
  assign out = a + b;
endmodule

// CHECK: (let v0 (Wire "v0" 1))
// CHECK: (let v1 (Wire "v1" 1))
// CHECK: (let v2 (Wire "v2" 1))
// CHECK: (union v2 (Op2 (Add) v0 v1))
// CHECK: (let a (Var "a" 1))
// CHECK: (IsPort "" "a" (Input) a)
// CHECK: (union v0 a)
// CHECK: (let b (Var "b" 1))
// CHECK: (IsPort "" "b" (Input) b)
// CHECK: (union v1 b)
// CHECK: (let out v2)
// CHECK: (IsPort "" "out" (Output) out)
// CHECK: (delete (Wire "v0" 1))
// CHECK: (delete (Wire "v1" 1))
// CHECK: (delete (Wire "v2" 1))
