// Copyright 2023 Intel Corporation.
//
// This reference design file is subject licensed to you by the terms and
// conditions of the applicable License Terms and Conditions for Hardware
// Reference Designs and/or Design Examples (either as signed by you or
// found at https://www.altera.com/common/legal/leg-license_agreement.html ).
//
// As stated in the license, you agree to only use this reference design
// solely in conjunction with Intel FPGAs or Intel CPLDs.
//
// THE REFERENCE DESIGN IS PROVIDED "AS IS" WITHOUT ANY EXPRESS OR IMPLIED
// WARRANTY OF ANY KIND INCLUDING WARRANTIES OF MERCHANTABILITY,
// NONINFRINGEMENT, OR FITNESS FOR A PARTICULAR PURPOSE. Intel does not
// warrant or assume responsibility for the accuracy or completeness of any
// information, links or other items within the Reference Design and any
// accompanying materials.
//
// In the event that you do not agree with such terms and conditions, do not
// use the reference design file.
/////////////////////////////////////////////////////////////////////////////
//
// TODO(@gussmith23): are we in line with this license?

// RUN: $YOSYS -q -m $CHURCHROAD_DIR/yosys-plugin/churchroad.so \
// RUN:   -p 'read_verilog -sv %s; prep -top permuter_4x4_sim; pmuxtree; prep; write_churchroad' \
// RUN:   | FileCheck %s

module permuter_4x4_sim #(
    parameter SIZE = 4
) (
	input clk,
	input [4*SIZE-1:0] din,
	input [1:0] control,
	output reg [4*SIZE-1:0] dout
);


always @(posedge clk) begin
    case (control)
        2'b00: begin
            {dout[(3+1)*SIZE-1:3*SIZE], dout[(2+1)*SIZE-1:2*SIZE], dout[(1+1)*SIZE-1:1*SIZE], dout[(0+1)*SIZE-1:0*SIZE]} <= {din[(3+1)*SIZE-1:3*SIZE], din[(2+1)*SIZE-1:2*SIZE], din[(1+1)*SIZE-1:1*SIZE], din[(0+1)*SIZE-1:0*SIZE]};
        end
        2'b01: begin
            {dout[(3+1)*SIZE-1:3*SIZE], dout[(2+1)*SIZE-1:2*SIZE], dout[(1+1)*SIZE-1:1*SIZE], dout[(0+1)*SIZE-1:0*SIZE]} <= {din[(2+1)*SIZE-1:2*SIZE], din[(3+1)*SIZE-1:3*SIZE], din[(0+1)*SIZE-1:0*SIZE], din[(1+1)*SIZE-1:1*SIZE]};
        end
        2'b10: begin
            {dout[(3+1)*SIZE-1:3*SIZE], dout[(2+1)*SIZE-1:2*SIZE], dout[(1+1)*SIZE-1:1*SIZE], dout[(0+1)*SIZE-1:0*SIZE]} <= {din[(1+1)*SIZE-1:1*SIZE], din[(0+1)*SIZE-1:0*SIZE], din[(3+1)*SIZE-1:3*SIZE], din[(2+1)*SIZE-1:2*SIZE]};
        end
        2'b11: begin
            {dout[(3+1)*SIZE-1:3*SIZE], dout[(2+1)*SIZE-1:2*SIZE], dout[(1+1)*SIZE-1:1*SIZE], dout[(0+1)*SIZE-1:0*SIZE]} <= {din[(0+1)*SIZE-1:0*SIZE], din[(1+1)*SIZE-1:1*SIZE], din[(2+1)*SIZE-1:2*SIZE], din[(3+1)*SIZE-1:3*SIZE]};
        end
    endcase
end

endmodule

// CHECK: (let v0 (Wire "v0" 16))
// CHECK: (let v1 (Wire "v1" 1))
// CHECK: (let v2 (Wire "v2" 16))
// CHECK: (let v3 (Wire "v3" 16))
// CHECK: (let v4 (Wire "v4" 8))
// CHECK: (let v5 (Wire "v5" 12))
// CHECK: (let v6 (Wire "v6" 16))
// CHECK: (let v7 (Wire "v7" 16))
// CHECK: (let v8 (Wire "v8" 8))
// CHECK: (let v9 (Wire "v9" 12))
// CHECK: (let v10 (Wire "v10" 16))
// CHECK: (let v11 (Wire "v11" 4))
// CHECK: (let v12 (Wire "v12" 4))
// CHECK: (let v13 (Wire "v13" 4))
// CHECK: (let v14 (Wire "v14" 4))
// CHECK: (let v15 (Wire "v15" 8))
// CHECK: (let v16 (Wire "v16" 8))
// CHECK: (let v17 (Wire "v17" 1))
// CHECK: (let v18 (Wire "v18" 1))
// CHECK: (let v19 (Wire "v19" 1))
// CHECK: (let v20 (Wire "v20" 1))
// CHECK: (let v21 (Wire "v21" 2))
// CHECK: (let v22 (Wire "v22" 16))
// CHECK: (let v23 (Wire "v23" 16))
// CHECK: (union v1 (Op2 (Or) v18 v17))
// CHECK: (union v2 (Op3 (Mux) v19 v22 v6))
// CHECK: (union v0 (Op3 (Mux) v1 v2 v3))
// CHECK: (union v3 (Op3 (Mux) v17 v7 v10))
// CHECK: (union v4 (Op2 (Concat) v12 v11))
// CHECK: (union v5 (Op2 (Concat) v13 v4))
// CHECK: (union v6 (Op2 (Concat) v14 v5))
// CHECK: (union v7 (Op2 (Concat) v16 v15))
// CHECK: (union v8 (Op2 (Concat) v14 v13))
// CHECK: (union v9 (Op2 (Concat) v11 v8))
// CHECK: (union v10 (Op2 (Concat) v12 v9))
// CHECK: (union v11 (Op1 (Extract 7 4) v22))
// CHECK: (union v12 (Op1 (Extract 3 0) v22))
// CHECK: (union v13 (Op1 (Extract 15 12) v22))
// CHECK: (union v14 (Op1 (Extract 11 8) v22))
// CHECK: (union v15 (Op1 (Extract 15 8) v22))
// CHECK: (union v16 (Op1 (Extract 7 0) v22))
// CHECK: (union v23 (Op2 (Reg 0) v20 v0))
// CHECK: (let v24 (Op0 (BV 3 2)))
// CHECK: (union v17 (Op2 (Eq) v21 v24))
// CHECK: (let v25 (Op0 (BV 2 2)))
// CHECK: (union v18 (Op2 (Eq) v21 v25))
// CHECK: (let v26 (Op0 (BV 1 1)))
// CHECK: (let v27 (Op1 (ZeroExtend 2) v26))
// CHECK: (union v19 (Op2 (Eq) v21 v27))
// CHECK: (let clk (Var "clk" 1))
// CHECK: (IsPort "" "clk" (Input) clk)
// CHECK: (union v20 clk)
// CHECK: (let control (Var "control" 2))
// CHECK: (IsPort "" "control" (Input) control)
// CHECK: (union v21 control)
// CHECK: (let din (Var "din" 16))
// CHECK: (IsPort "" "din" (Input) din)
// CHECK: (union v22 din)
// CHECK: (let dout v23)
// CHECK: (IsPort "" "dout" (Output) dout)
// CHECK: (delete (Wire "v0" 16))
// CHECK: (delete (Wire "v1" 1))
// CHECK: (delete (Wire "v2" 16))
// CHECK: (delete (Wire "v3" 16))
// CHECK: (delete (Wire "v4" 8))
// CHECK: (delete (Wire "v5" 12))
// CHECK: (delete (Wire "v6" 16))
// CHECK: (delete (Wire "v7" 16))
// CHECK: (delete (Wire "v8" 8))
// CHECK: (delete (Wire "v9" 12))
// CHECK: (delete (Wire "v10" 16))
// CHECK: (delete (Wire "v11" 4))
// CHECK: (delete (Wire "v12" 4))
// CHECK: (delete (Wire "v13" 4))
// CHECK: (delete (Wire "v14" 4))
// CHECK: (delete (Wire "v15" 8))
// CHECK: (delete (Wire "v16" 8))
// CHECK: (delete (Wire "v17" 1))
// CHECK: (delete (Wire "v18" 1))
// CHECK: (delete (Wire "v19" 1))
// CHECK: (delete (Wire "v20" 1))
// CHECK: (delete (Wire "v21" 2))
// CHECK: (delete (Wire "v22" 16))
// CHECK: (delete (Wire "v23" 16))
