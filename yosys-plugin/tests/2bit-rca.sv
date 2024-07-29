// RUN: $YOSYS -q -m $CHURCHROAD_DIR/yosys-plugin/churchroad.so \
// RUN:   -p 'read_verilog -sv %s; prep -top top; pmuxtree; prep; write_churchroad' \
// RUN:   | FileCheck %s

module top
  (
	  i_a,
	  i_b,
	  o_s,
	  o_c
  );

  input [1:0] i_a;
  input [1:0] i_b;
  output [1:0] o_s;
  output o_c;

  wire [1:0] c;
  
  assign o_s[0] = i_a[0] ^ i_b[0] ^ 1'b0;
  assign c[0] = (i_a[0] & i_b[0]) | (i_b[0] & 1'b0) | (i_a[0] & 1'b0);
  
  assign o_s[1] = i_a[1] ^ i_b[1] ^ c[0];
  assign c[1] = (i_a[1] & i_b[1]) | (i_b[1] & c[0]) | (i_a[1] & c[0]);
  
  assign o_c = c[1];

endmodule

// CHECK: (let v0 (Wire "v0" 1))
// CHECK: (let v1 (Wire "v1" 1))
// CHECK: (let v2 (Wire "v2" 1))
// CHECK: (let v3 (Wire "v3" 1))
// CHECK: (let v4 (Wire "v4" 1))
// CHECK: (let v5 (Wire "v5" 1))
// CHECK: (let v6 (Wire "v6" 1))
// CHECK: (let v7 (Wire "v7" 1))
// CHECK: (let v8 (Wire "v8" 1))
// CHECK: (let v9 (Wire "v9" 1))
// CHECK: (let v10 (Wire "v10" 1))
// CHECK: (let v11 (Wire "v11" 1))
// CHECK: (let v12 (Wire "v12" 1))
// CHECK: (let v13 (Wire "v13" 2))
// CHECK: (let v14 (Wire "v14" 2))
// CHECK: (let v15 (Wire "v15" 2))
// CHECK: (union v11 (Op2 (And) v5 v6))
// CHECK: (union v0 (Op2 (And) v7 v8))
// CHECK: (union v1 (Op2 (And) v8 v11))
// CHECK: (union v2 (Op2 (And) v7 v11))
// CHECK: (union v15 (Op2 (Concat) v4 v3))
// CHECK: (union v5 (Op1 (Extract 0 0) v13))
// CHECK: (union v6 (Op1 (Extract 0 0) v14))
// CHECK: (union v7 (Op1 (Extract 1 1) v13))
// CHECK: (union v8 (Op1 (Extract 1 1) v14))
// CHECK: (union v9 (Op2 (Or) v0 v1))
// CHECK: (union v12 (Op2 (Or) v9 v2))
// CHECK: (union v3 (Op2 (Xor) v5 v6))
// CHECK: (union v10 (Op2 (Xor) v7 v8))
// CHECK: (union v4 (Op2 (Xor) v10 v11))
// CHECK: (let i_a (Var "i_a" 2))
// CHECK: (IsPort "" "i_a" (Input) i_a)
// CHECK: (union v13 i_a)
// CHECK: (let i_b (Var "i_b" 2))
// CHECK: (IsPort "" "i_b" (Input) i_b)
// CHECK: (union v14 i_b)
// CHECK: (let o_c v12)
// CHECK: (IsPort "" "o_c" (Output) o_c)
// CHECK: (let o_s v15)
// CHECK: (IsPort "" "o_s" (Output) o_s)
// CHECK: (delete (Wire "v0" 1))
// CHECK: (delete (Wire "v1" 1))
// CHECK: (delete (Wire "v2" 1))
// CHECK: (delete (Wire "v3" 1))
// CHECK: (delete (Wire "v4" 1))
// CHECK: (delete (Wire "v5" 1))
// CHECK: (delete (Wire "v6" 1))
// CHECK: (delete (Wire "v7" 1))
// CHECK: (delete (Wire "v8" 1))
// CHECK: (delete (Wire "v9" 1))
// CHECK: (delete (Wire "v10" 1))
// CHECK: (delete (Wire "v11" 1))
// CHECK: (delete (Wire "v12" 1))
// CHECK: (delete (Wire "v13" 2))
// CHECK: (delete (Wire "v14" 2))
// CHECK: (delete (Wire "v15" 2))
