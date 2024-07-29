// RUN: $YOSYS -m $CHURCHROAD_DIR/yosys-plugin/churchroad.so \
// RUN:   -p 'read_verilog -sv %s; hierarchy -top top; write_churchroad' \
// RUN: | FileCheck %s

module top(a, b, out);
  wire [47:0] P_0;
  input [15:0] a;
  wire [15:0] a;
  input [15:0] b;
  wire [15:0] b;
  output [15:0] out;
  wire [15:0] out;
  DSP48E2 #(
    .ACASCREG(32'd0),
    .ADREG(32'd0),
    .ALUMODEREG(32'd0),
    .AMULTSEL("AD"),
    .AREG(32'd0),
    .AUTORESET_PATDET("NO_RESET"),
    .AUTORESET_PRIORITY("RESET"),
    .A_INPUT("DIRECT"),
    .BCASCREG(32'd0),
    .BMULTSEL("B"),
    .BREG(32'd0),
    .B_INPUT("DIRECT"),
    .CARRYINREG(32'd0),
    .CARRYINSELREG(32'd1),
    .CREG(32'd0),
    .DREG(32'd0),
    .INMODEREG(32'd1),
    .IS_ALUMODE_INVERTED(4'h0),
    .IS_CARRYIN_INVERTED(1'h0),
    .IS_CLK_INVERTED(1'h0),
    .IS_INMODE_INVERTED(5'h00),
    .IS_OPMODE_INVERTED(9'h000),
    .IS_RSTALLCARRYIN_INVERTED(1'h0),
    .IS_RSTALUMODE_INVERTED(1'h0),
    .IS_RSTA_INVERTED(1'h0),
    .IS_RSTB_INVERTED(1'h0),
    .IS_RSTCTRL_INVERTED(1'h0),
    .IS_RSTC_INVERTED(1'h0),
    .IS_RSTD_INVERTED(1'h0),
    .IS_RSTINMODE_INVERTED(1'h0),
    .IS_RSTM_INVERTED(1'h0),
    .IS_RSTP_INVERTED(1'h0),
    .MASK(48'h000000000000),
    .MREG(32'd0),
    .OPMODEREG(32'd0),
    .PATTERN(48'h000000000000),
    .PREADDINSEL("A"),
    .PREG(32'd0),
    .RND(48'h000000000000),
    .SEL_MASK("MASK"),
    .SEL_PATTERN("PATTERN"),
    .USE_MULT("MULTIPLY"),
    .USE_PATTERN_DETECT("NO_PATDET"),
    .USE_SIMD("ONE48"),
    .USE_WIDEXOR("FALSE"),
    .XORSIMD("XOR12")
  ) DSP48E2_0 (
    .A({ a[15], a[15], a[15], a[15], a[15], a[15], a[15], a[15], a[15], a[15], a[15], a[15], a[15], a[15], a }),
    .ACIN(30'h00000000),
    .ALUMODE(4'h7),
    .B({ b[15], b[15], b }),
    .BCIN(18'h00000),
    .C(48'h000000000000),
    .CARRYCASCIN(1'h0),
    .CARRYIN(1'h0),
    .CARRYINSEL(3'h6),
    .CEA1(1'h1),
    .CEA2(1'h1),
    .CEAD(1'h1),
    .CEALUMODE(1'h1),
    .CEB1(1'h1),
    .CEB2(1'h1),
    .CEC(1'h1),
    .CECARRYIN(1'h1),
    .CECTRL(1'h1),
    .CED(1'h1),
    .CEINMODE(1'h1),
    .CEM(1'h1),
    .CEP(1'h1),
    .CLK(1'h0),
    .D(27'h0000000),
    .INMODE(5'h1b),
    .MULTSIGNIN(1'h0),
    .OPMODE(9'h035),
    .P({ P_0[47:16], out }),
    .PCIN(48'h000000000000),
    .RSTA(1'h0),
    .RSTALLCARRYIN(1'h0),
    .RSTALUMODE(1'h0),
    .RSTB(1'h0),
    .RSTC(1'h0),
    .RSTCTRL(1'h0),
    .RSTD(1'h0),
    .RSTINMODE(1'h0),
    .RSTM(1'h0),
    .RSTP(1'h0)
  );
  assign P_0[15] = out[15];
  assign P_0[14] = out[14];
  assign P_0[13] = out[13];
  assign P_0[12] = out[12];
  assign P_0[11] = out[11];
  assign P_0[10] = out[10];
  assign P_0[9] = out[9];
  assign P_0[8] = out[8];
  assign P_0[7] = out[7];
  assign P_0[6] = out[6];
  assign P_0[5] = out[5];
  assign P_0[4] = out[4];
  assign P_0[3] = out[3];
  assign P_0[2] = out[2];
  assign P_0[1] = out[1];
  assign P_0[0] = out[0];
endmodule

// DSP48E2 definition from Xilinx. Note that we keep the module empty for now --
// we only need the input/output port definitions.

///////////////////////////////////////////////////////////////////////////////
//  Copyright (c) 1995/2018 Xilinx, Inc.
//  All Right Reserved.
///////////////////////////////////////////////////////////////////////////////
//   ____  ____
//  /   /\/   /
// /___/  \  /     Vendor      : Xilinx
// \   \   \/      Version     : 2018.3
//  \   \          Description : Xilinx Unified Simulation Library Component
//  /   /                        48-bit Multi-Functional Arithmetic Block
// /___/   /\      Filename    : DSP48E2.v
// \   \  /  \
//  \___\/\___\
//
///////////////////////////////////////////////////////////////////////////////
//  Revision:
//  07/15/12 - Migrate from E1.
//  12/10/12 - Add dynamic registers
//  01/10/13 - 694456 - DIN_in/D_in connectivity issue
//  01/11/13 - DIN, D_DATA data width change (26/24) sync4 yml
//  02/13/13 - PCIN_47A change from internal feedback to PCIN(47) pin
//  03/06/13 - 701316 - A_B_reg no clk when REG=0
//  04/03/13 - yaml update
//  04/08/13 - 710304 - AREG, BREG, ACASCREG and BCASCREG dynamic registers mis sized.
//  04/22/13 - 714213 - ACOUT, BCOUT wrong logic
//  04/22/13 - 713695 - Zero mult result on USE_SIMD
//  04/22/13 - 713617 - CARRYCASCOUT behaviour
//  04/23/13 - 714772 - remove sensitivity to negedge GSR
//  04/23/13 - 713706 - change P_PDBK connection
//  05/07/13 - 716896 - AREG, BREG, ACASCREG and BCASCREG localparams mis sized.
//  05/07/13 - 716896 - ALUMODE/OPMODE_INV_REG mis sized
//  05/07/13 - 716896 - INMODE_INV_REG mis sized
//  05/07/13 - x_mac_cascd missing for sensitivity list.
//  10/22/14 - 808642 - Added #1 to $finish
//  End Revision:
///////////////////////////////////////////////////////////////////////////////

module DSP48E2 #(
    parameter integer ACASCREG = 1,
    parameter integer ADREG = 1,
    parameter integer ALUMODEREG = 1,
    parameter AMULTSEL = "A",
    parameter integer AREG = 1,
    parameter AUTORESET_PATDET = "NO_RESET",
    parameter AUTORESET_PRIORITY = "RESET",
    parameter A_INPUT = "DIRECT",
    parameter integer BCASCREG = 1,
    parameter BMULTSEL = "B",
    parameter integer BREG = 1,
    parameter B_INPUT = "DIRECT",
    parameter integer CARRYINREG = 1,
    parameter integer CARRYINSELREG = 1,
    parameter integer CREG = 1,
    parameter integer DREG = 1,
    parameter integer INMODEREG = 1,
    parameter [3:0] IS_ALUMODE_INVERTED = 4'b0000,
    parameter [0:0] IS_CARRYIN_INVERTED = 1'b0,
    parameter [0:0] IS_CLK_INVERTED = 1'b0,
    parameter [4:0] IS_INMODE_INVERTED = 5'b00000,
    parameter [8:0] IS_OPMODE_INVERTED = 9'b000000000,
    parameter [0:0] IS_RSTALLCARRYIN_INVERTED = 1'b0,
    parameter [0:0] IS_RSTALUMODE_INVERTED = 1'b0,
    parameter [0:0] IS_RSTA_INVERTED = 1'b0,
    parameter [0:0] IS_RSTB_INVERTED = 1'b0,
    parameter [0:0] IS_RSTCTRL_INVERTED = 1'b0,
    parameter [0:0] IS_RSTC_INVERTED = 1'b0,
    parameter [0:0] IS_RSTD_INVERTED = 1'b0,
    parameter [0:0] IS_RSTINMODE_INVERTED = 1'b0,
    parameter [0:0] IS_RSTM_INVERTED = 1'b0,
    parameter [0:0] IS_RSTP_INVERTED = 1'b0,
    parameter [47:0] MASK = 48'h3FFFFFFFFFFF,
    parameter integer MREG = 1,
    parameter integer OPMODEREG = 1,
    parameter [47:0] PATTERN = 48'h000000000000,
    parameter PREADDINSEL = "A",
    parameter integer PREG = 1,
    parameter [47:0] RND = 48'h000000000000,
    parameter SEL_MASK = "MASK",
    parameter SEL_PATTERN = "PATTERN",
    parameter USE_MULT = "MULTIPLY",
    parameter USE_PATTERN_DETECT = "NO_PATDET",
    parameter USE_SIMD = "ONE48",
    parameter USE_WIDEXOR = "FALSE",
    parameter XORSIMD = "XOR24_48_96"
) (
    output [29:0] ACOUT,
    output [17:0] BCOUT,
    output CARRYCASCOUT,
    output [3:0] CARRYOUT,
    output MULTSIGNOUT,
    output OVERFLOW,
    output [47:0] P,
    output PATTERNBDETECT,
    output PATTERNDETECT,
    output [47:0] PCOUT,
    output UNDERFLOW,
    output [7:0] XOROUT,

    input [29:0] A,
    input [29:0] ACIN,
    input [3:0] ALUMODE,
    input [17:0] B,
    input [17:0] BCIN,
    input [47:0] C,
    input CARRYCASCIN,
    input CARRYIN,
    input [2:0] CARRYINSEL,
    input CEA1,
    input CEA2,
    input CEAD,
    input CEALUMODE,
    input CEB1,
    input CEB2,
    input CEC,
    input CECARRYIN,
    input CECTRL,
    input CED,
    input CEINMODE,
    input CEM,
    input CEP,
    input CLK,
    input [26:0] D,
    input [4:0] INMODE,
    input MULTSIGNIN,
    input [8:0] OPMODE,
    input [47:0] PCIN,
    input RSTA,
    input RSTALLCARRYIN,
    input RSTALUMODE,
    input RSTB,
    input RSTC,
    input RSTCTRL,
    input RSTD,
    input RSTINMODE,
    input RSTM,
    input RSTP
);

endmodule

// CHECK: (let DSP48E2_0 (ModuleInstance "DSP48E2" (vec-of "A" "ACIN" "ALUMODE" "B" "BCIN" "C" "CARRYCASCIN" "CARRYIN" "CARRYINSEL" "CEA1" "CEA2" "CEAD" "CEALUMODE" "CEB1" "CEB2" "CEC" "CECARRYIN" "CECTRL" "CED" "CEINMODE" "CEM" "CEP" "CLK" "D" "INMODE" "MULTSIGNIN" "OPMODE" "PCIN" "RSTA" "RSTALLCARRYIN" "RSTALUMODE" "RSTB" "RSTC" "RSTCTRL" "RSTD" "RSTINMODE" "RSTM" "RSTP") (vec-of v13 v24 v25 v15 v26 v27 v28 v28 v29 v30 v30 v30 v30 v30 v30 v30 v30 v30 v30 v30 v30 v30 v28 v31 v32 v28 v33 v27 v28 v28 v28 v28 v28 v28 v28 v28 v28 v28)))