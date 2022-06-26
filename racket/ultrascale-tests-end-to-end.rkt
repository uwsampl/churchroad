#lang errortrace racket

(require "compile-to-json.rkt"
         "verilator.rkt"
         json
         rosette
         rosette/lib/synthax
         "interpreter.rkt"
         "programs-to-synthesize.rkt"
         "circt-comb-operators.rkt"
         "utils.rkt"
         "synthesize.rkt")

(define (end-to-end-test bv-expr)
  (simulate-expr (synthesize-xilinx-ultrascale-plus-impl bv-expr) bv-expr))

(module+ test
  (require rackunit)

  ;;; TODO for now these need to be named l0..l5. Make this more flexible.
  (define-symbolic l0 l1 (bitvector 8))

  (check-true (end-to-end-test (bvand l0 l1)))
  (check-true (end-to-end-test (bvxor l0 l1)))
  (check-true (end-to-end-test (bvor l0 l1)))
  (check-true (end-to-end-test (bvadd l0 l1)))
  (check-true (end-to-end-test (bvsub l0 l1)))
  (check-true (end-to-end-test (bithack1 l0 l1)))
  (check-true (end-to-end-test (bithack2 l0 l1)))
  (check-true (end-to-end-test (bithack3 l0 l1)))
  (check-true (end-to-end-test l0))
  (check-true (end-to-end-test (bvmul l0 (bv 0 8))))
  (check-true (end-to-end-test (bvmul l0 (bv 1 8))))
  (check-true (end-to-end-test (bvmul l0 (bv 2 8))))
  (check-true (end-to-end-test (circt-comb-shl l0 (bv 0 8))))
  (check-true (end-to-end-test (circt-comb-shl l0 (bv 1 8)))))