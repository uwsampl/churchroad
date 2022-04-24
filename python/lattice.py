from z3 import *


def lut4(init, A, B, C, D):
    tmp = If(A, Extract(15, 8, init), Extract(7, 0, init))
    tmp = If(B, Extract(7, 4, tmp), Extract(3, 0, tmp))
    tmp = If(C, Extract(3, 2, tmp), Extract(1, 0, tmp))
    tmp = If(D, Extract(1, 1, tmp), Extract(0, 0, tmp))
    return tmp


def mux(C, A, B):
    return If(C, A, B)


def extract_to_bool_array(high, low, x):
    ret = []
    for i in range(high, low - 1, -1):
        xi = Extract(i, i, x)
        b = xi == BitVecVal(1, 1)
        ret.append(b)
    return ret


class LUT4:
    def __init__(self, prefix):
        self.prefix = prefix
        self.mem = BitVec(prefix + "_init", 16)

    def __call__(self, A, B, C, D):
        return lut4(self.mem, A, B, C, D)


class MUX:
    def __init__(self, prefix):
        self.prefix = prefix
        self.selector = Bool(prefix + "_selector")

    def __call__(self, A, B):
        return mux(self.selector, A, B)


class Slice:
    def __init__(self, prefix, solver):
        self.prefix = prefix
        self.solver = solver
        self.lut4_0 = LUT4(prefix + "_lut_0")
        self.lut4_1 = LUT4(prefix + "_lut_1")
        self.pfumx = MUX(prefix + "_PFUMX")
        self.l6mux21 = MUX(prefix + "_L6MUX21")

    def __call__(self, inputs, FXA, FXB):
        F0 = self.lut4_0(*extract_to_bool_array(7, 4, inputs))
        F1 = self.lut4_1(*extract_to_bool_array(3, 0, inputs))
        OFX0 = self.pfumx(F0, F1)
        OFX1 = self.l6mux21(FXA, FXB)
        return F0, F1, OFX0, OFX1
