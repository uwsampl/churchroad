/*
 *  yosys -- Yosys Open SYnthesis Suite
 *
 *  Copyright (C) 2012  Claire Xenia Wolf <claire@yosyshq.com>
 *
 *  Permission to use, copy, modify, and/or distribute this software for any
 *  purpose with or without fee is hereby granted, provided that the above
 *  copyright notice and this permission notice appear in all copies.
 *
 *  THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES
 *  WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF
 *  MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR
 *  ANY SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
 *  WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN
 *  ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF
 *  OR IN CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.
 *
 */

#include "kernel/celltypes.h"
#include "kernel/json.h"
#include "kernel/log.h"
#include "kernel/mem.h"
#include "kernel/register.h"
#include "kernel/rtlil.h"
#include "kernel/sigtools.h"
#include "kernel/yw.h"
#include <string>
#include <cassert>

USING_YOSYS_NAMESPACE
PRIVATE_NAMESPACE_BEGIN

struct LakeroadWorker
{
	std::string salt;
	// Whether or not to generate let bindings for ports.
	bool let_bindings;
	std::map<std::string, std::string> port_to_expr_map;
	std::ostream &f;
	SigMap sigmap;
	RTLIL::Module *module;
	bool verbose;
	bool single_bad;
	bool cover_mode;
	bool print_internal_names;

	int next_nid = 1;
	int initstate_nid = -1;

	// <width> => <sid>
	dict<int, int> sorts_bv;

	// (<address-width>, <data-width>) => <sid>
	dict<pair<int, int>, int> sorts_mem;

	// SigBit => (<nid>, <bitidx>)
	dict<SigBit, pair<int, int>> bit_nid;

	// <nid> => <bvwidth>
	dict<int, int> nid_width;

	// SigSpec => <nid>
	dict<SigSpec, int> sig_nid;

	// bit to driving cell
	dict<SigBit, Cell *> bit_cell;

	// nids for constants
	dict<Const, int> consts;

	// ff inputs that need to be evaluated (<nid>, <ff_cell>)
	vector<pair<int, Cell *>> ff_todo;
	vector<pair<int, Mem *>> mem_todo;

	pool<Cell *> cell_recursion_guard;
	vector<int> bad_properties;
	dict<SigBit, bool> initbits;
	pool<Wire *> statewires;
	pool<string> srcsymbols;
	vector<Mem> memories;
	dict<Cell *, Mem *> mem_cells;

	string indent, info_filename;
	vector<string> info_lines;
	dict<int, int> info_clocks;

	struct ywmap_btor_sig
	{
		SigSpec sig;
		Cell *cell = nullptr;

		ywmap_btor_sig(const SigSpec &sig) : sig(sig) {}
		ywmap_btor_sig(Cell *cell) : cell(cell) {}
	};

	vector<ywmap_btor_sig> ywmap_inputs;
	vector<ywmap_btor_sig> ywmap_states;
	dict<SigBit, int> ywmap_clock_bits;
	dict<SigBit, int> ywmap_clock_inputs;

	PrettyJson ywmap_json;

	void btorf(const char *fmt, ...) YS_ATTRIBUTE(format(printf, 2, 3))
	{
		va_list ap;
		va_start(ap, fmt);
		f << indent << vstringf(fmt, ap);
		va_end(ap);
	}

	void infof(const char *fmt, ...) YS_ATTRIBUTE(format(printf, 2, 3))
	{
		va_list ap;
		va_start(ap, fmt);
		info_lines.push_back(vstringf(fmt, ap));
		va_end(ap);
	}

	template <typename T>
	string getinfo(T *obj, bool srcsym = false)
	{
		string infostr = log_id(obj);
		if (!srcsym && !print_internal_names && infostr[0] == '$')
			return "";
		if (obj->attributes.count(ID::src))
		{
			string src = obj->attributes.at(ID::src).decode_string().c_str();
			if (srcsym && infostr[0] == '$')
			{
				std::replace(src.begin(), src.end(), ' ', '_');
				if (srcsymbols.count(src) || module->count_id("\\" + src))
				{
					for (int i = 1;; i++)
					{
						string s = stringf("%s-%d", src.c_str(), i);
						if (!srcsymbols.count(s) && !module->count_id("\\" + s))
						{
							src = s;
							break;
						}
					}
				}
				srcsymbols.insert(src);
				infostr = src;
			}
			else
			{
				infostr += " ; " + src;
			}
		}
		return " " + infostr;
	}

	LakeroadWorker(std::ostream &f, RTLIL::Module *module, std::string &salt, bool let_bindings, std::map<std::string, std::string> port_to_expr_map) : salt(salt), let_bindings(let_bindings), port_to_expr_map(port_to_expr_map), f(f), sigmap(module), module(module) {}

	void run()
	{
		if (module->processes.size() > 0)
			log_error("Module has processes.\n");

		// IDs used to generate let expressions.
		int id = 0;
		auto get_new_id_str = [&]()
		{ return stringf("v%d%s%s", id++, salt.empty() ? "" : "-", salt.c_str()); };

		// Get signal name.
		auto get_signal_name = [&](const SigSpec &sig)
		{
			auto signal_name = std::string(log_signal(sig));
			assert(signal_name.size() > 0);
			if (signal_name[0] == '\\')
				signal_name = signal_name.substr(1);
			return signal_name;
		};

		// Function to generate a let expression.
		auto let = [&](const std::string &id_str, const std::string &expr)
		{ return stringf("(let %s %s)", id_str.c_str(), expr.c_str()); };

		// Wire expressions (which we can eventually delete)
		vector<std::string> wire_exprs;

		// Generate wire expression.
		auto wire_expr = [&](const std::string &name, const int bitwidth)
		{
			auto s = stringf("(Wire \"%s\" %d)", name.c_str(), bitwidth);
			wire_exprs.push_back(s);
			return s;
		};

		std::map<RTLIL::SigSpec, std::string> signal_let_bound_name;

		// Note: we `sigmap` all wires here, so there's no need to `sigmap` them
		// recursively within get_expression_for_signal. This was a source of an
		// infinite loop: https://github.com/uwsampl/yosys/issues/10
		//
		// Does not sigmap the signal; you should sigmap the signal if you need it
		// sigmapped.
		//
		// to_width = -1 means no extension.
		std::function<std::string(const SigSpec &, int)> get_expression_for_signal = [&](const SigSpec &sig, int to_width)
		{
			std::function<std::string(std::string, const SigSpec &, int)> convert_to_width = [&](std::string expr, const SigSpec &sig, int to_width)
			{
				auto out_expr = expr;

				// If we need to extend the signal, do so.
				if (to_width >= 0 && to_width != GetSize(sig))
				{
					if (to_width < GetSize(sig))
					{
						auto new_id = get_new_id_str();
						auto extend_expr = stringf("(Op1 (Extract %d %d) %s)", to_width - 1, 0, expr.c_str());
						f << let(new_id, extend_expr) << "\n";
						out_expr = new_id;
					}
					else
					{

						auto new_id = get_new_id_str();
						f << "; TODO not handling signedness\n";
						auto extend_expr = stringf("(Op1 (ZeroExtend %d) %s)", to_width, expr.c_str());
						f << let(new_id, extend_expr) << "\n";
						out_expr = new_id;
					}
				}

				return out_expr;
			};

			// If we've already handled the expression, return it.
			if (signal_let_bound_name.count(sig))
				return convert_to_width(signal_let_bound_name.at(sig), sig, to_width);

			// SigSpecs are either constants, wires, or concatenations and selections
			// of wires. We simply need to handle each case.

			// The output expression will go in here.
			std::string out_expr;

			if (sig.is_fully_const())
			{
				// If the signal is a constant, we can just use the constant.
				auto const_str = stringf("(Op0 (BV %d %d))", Const(sig.as_const()).as_int(), sig.size());
				auto new_id = get_new_id_str();
				auto let_expr = let(new_id, const_str);
				auto signal_name = get_signal_name(sig);
				f << "; " << signal_name << "\n";
				f << let_expr << "\n";
				out_expr = new_id;
			}
			else if (sig.is_wire())
			{
				// If the signal is a simple wire, we can just use the name of the wire.
				auto signal_name = get_signal_name(sig);
				auto let_bound_id = get_new_id_str();
				// Generate wire expression
				auto let_expr = let(let_bound_id, wire_expr(let_bound_id, GetSize(sig)));
				f << "; " << signal_name << "\n";
				f << let_expr << "\n";
				out_expr = let_bound_id;
			}
			else if (sig.chunks().size() > 1)
			{
				// If the signal is multiple chunks, we need to generate a concatenation.

				f << "; " << log_signal(sig) << "\n";

				// Generate expression for each chunk.
				std::vector<std::string> chunk_exprs;
				for (auto chunk : sig.chunks())
				{
					chunk_exprs.push_back(get_expression_for_signal(chunk, -1));
				}

				// Generate concatenation expressions.
				auto concat_expr = chunk_exprs[0];
				for (size_t i = 1; i < chunk_exprs.size(); i++)
				{
					auto new_id = get_new_id_str();
					auto let_expr = let(new_id, stringf("(Op2 (Concat) %s %s)", concat_expr.c_str(), chunk_exprs[i].c_str()));
					f << let_expr << "\n";
					concat_expr = new_id;
				}

				out_expr = concat_expr;
			}
			else if (sig.chunks().size() == 1 && sig.chunks()[0].wire->width != sig.size())
			{
				// This branch is meant to capture the case where the signal is a
				// slice/extraction. I'm not quite sure how to check this in Yosys
				// though.
				//
				// It may be the case that we've set up these if statements in a way
				// that this is the only possible case that's left. That would be nice.
				// Currently, the condition in this else if branch is a little messy.

				auto chunk = sig.chunks()[0];

				if (chunk.wire->upto)
				{
					// You can copy implementation for upto from verilog_backend.cc.
					log_error("Unimplemented case of `upto==true` in %s.\n", log_signal(sig));
				}

				// The let-bound ID string of the expression to extract from.
				auto extract_from_expr = get_expression_for_signal(sig.chunks()[0].wire, -1);
				auto new_id = get_new_id_str();
				auto extract_expr = stringf("(Op1 (Extract %d %d) %s)", (chunk.offset + chunk.width - 1) + chunk.wire->start_offset,
																		chunk.offset + chunk.wire->start_offset, extract_from_expr.c_str());

				auto let_expr = let(new_id, extract_expr);
				f << let_expr << "\n";
				out_expr = new_id;
			}
			else
			{
				log_error("Unhandled case of signal for %s.\n", log_signal(sig));
			}

			signal_let_bound_name.insert({sig, out_expr});
			return convert_to_width(out_expr, sig, to_width);
		};

		// Create Wire expression for each wire.
		// We don't have to do this upfront anymore. Expressions can just be created
		// as needed.
		f << "; wire declarations\n";
		for (auto wire : module->wires())
		{
			get_expression_for_signal(sigmap(wire), -1);
		}

		// Handle cells
		f << "\n; cells\n";
		for (auto cell : module->cells())
		{

			if (cell->type.in(ID($logic_not), ID($not), ID($reduce_or), ID($reduce_bool), ID($reduce_and), ID($reduce_xor)))
			{
				// Unary ops.
				assert(cell->connections().size() == 2);
				auto y = sigmap(cell->getPort(ID::Y));
				auto a_let_name = get_expression_for_signal(sigmap(cell->getPort(ID::A)), y.size());
				auto y_let_name = get_expression_for_signal(y, -1);

				std::string op_str;
				if (cell->type == ID($logic_not))
					op_str = "(LogicNot)";
				else if (cell->type == ID($not))
					op_str = "(Not)";
				else if (cell->type.in(ID($reduce_or), ID($reduce_bool)))
					op_str = "(ReduceOr)";
				else if (cell->type == ID($reduce_and))
					op_str = "(ReduceAnd)";
				else if (cell->type == ID($reduce_xor))
					op_str = "(ReduceXor)";
				else
					log_error("This should be unreachable. You are missing an else if branch.\n");

				f << stringf("(union %s (Op1 %s %s))\n", y_let_name.c_str(), op_str.c_str(), a_let_name.c_str()).c_str();
			}
			else if (cell->type.in(ID($slice)))
			{
				// Slice.
				assert(cell->connections().size() == 2);
				// Do we need sigmap here?
				auto y = sigmap(cell->getPort(ID::Y));
				auto a_let_name = get_expression_for_signal(sigmap(cell->getPort(ID::A)), -1);
				auto y_let_name = get_expression_for_signal(y, -1);

				// Get OFFSET and Y_WIDTH parameters.
				auto offset = cell->getParam(ID::OFFSET).as_int();
				auto y_width = cell->getParam(ID::Y_WIDTH).as_int();

				std::string op_str;
				if (cell->type == ID($slice))
					op_str = stringf("(Extract %d %d)", offset + y_width - 1, offset).c_str();
				else
					log_error("This should be unreachable. You are missing an else if branch.\n");

				f << stringf("(union %s (Op1 %s %s))\n", y_let_name.c_str(), op_str.c_str(), a_let_name.c_str()).c_str();
			}
			else if (cell->type.in(ID($and), ID($or), ID($xor), ID($shr), ID($add), ID($shiftx), ID($mul), ID($sub)))
			{
				// Assert that A and B are both unsigned. Note that this is a
				// simplifying assumption. It does not have to be true, but supporting
				// different signedness would require some thought that I'm not putting
				// in right now.
				assert(cell->getParam(ID::A_SIGNED).is_fully_zero());
				assert(cell->getParam(ID::B_SIGNED).is_fully_zero());

				// Get the max width of the inputs. This determines the width we need to
				// extend both inputs to.
				auto max_width = std::max(cell->getPort(ID::A).size(), std::max(cell->getPort(ID::B).size(), cell->getPort(ID::Y).size()));

				assert(cell->connections().size() == 3);
				auto a_let_name = get_expression_for_signal(cell->getPort(ID::A), max_width);
				auto b_let_name = get_expression_for_signal(cell->getPort(ID::B), max_width);
				auto y_let_name = get_expression_for_signal(cell->getPort(ID::Y), -1);

				std::string op_str;
				if (cell->type == ID($and))
					op_str = "(And)";
				else if (cell->type == ID($or))
					op_str = "(Or)";
				else if (cell->type == ID($xor))
					op_str = "(Xor)";
				// Here, $shr and $shiftx are treated the same.
				// This is only true because we've asserted that A and B are unsigned.
				// See #26:
				// https://github.com/uwsampl/churchroad/issues/26
				else if (cell->type.in(ID($shr), ID($shiftx)))
					op_str = "(Shr)";
				else if (cell->type == ID($add))
					op_str = "(Add)";
				else if (cell->type == ID($mul))
					op_str = "(Mul)";
				else if (cell->type == ID($sub))
					op_str = "(Sub)";
				else
					log_error("This should be unreachable. You are missing an else if branch.\n");

				op_str = stringf("(Op2 %s %s %s)", op_str.c_str(), a_let_name.c_str(),
												 b_let_name.c_str());

				// If the output width is less than the result of the operation, we need
				// to slice the result.
				//
				// This is an assumption we're currently making. Doesn't have to be the
				// case. We may also need to extend the result in the future, when this
				// assertion fails.
				if (cell->getPort(ID::Y).size() < max_width)
				{
					op_str = stringf("(Op1 (Extract %d %d) %s)", cell->getPort(ID::Y).size() - 1, 0, op_str.c_str());
				}

				f << stringf("(union %s %s)\n", y_let_name.c_str(), op_str.c_str()).c_str();
			}
			else if (cell->type.in(ID($concat)))
			{
				// Concat.
				assert(cell->connections().size() == 3);
				// do we need this sigmap?
				auto y = sigmap(cell->getPort(ID::Y));
				auto a_let_name = get_expression_for_signal(sigmap(cell->getPort(ID::A)), -1);
				auto b_let_name = get_expression_for_signal(sigmap(cell->getPort(ID::B)), -1);
				auto y_let_name = get_expression_for_signal(y, -1);

				std::string op_str;
				if (cell->type == ID($concat))
					op_str = "(Concat)";
				else
					log_error("This should be unreachable. You are missing an else if branch.\n");

				// Here, we assume that the semantics of Yosys' $concat is Y = {B, A}.
				// https://www.reddit.com/r/yosys/comments/4e9co6/very_basic_question_about_cellmapping/
				f << stringf("(union %s (Op2 %s %s %s))\n", y_let_name.c_str(), op_str.c_str(), b_let_name.c_str(),
										 a_let_name.c_str())
								 .c_str();
			}
			else if (cell->type.in(ID($mux)))
			{
				assert(cell->connections().size() == 4);
				auto y = sigmap(cell->getPort(ID::Y));
				// TODO(@gussmith23): Should we cast to y.size() here?
				// If these fail, it's probably because splice/splitnets wasn't run.
				assert(cell->getPort(ID::A).size() == y.size());
				assert(cell->getPort(ID::B).size() == y.size());
				auto a_let_name = get_expression_for_signal(sigmap(cell->getPort(ID::A)), y.size());
				auto b_let_name = get_expression_for_signal(sigmap(cell->getPort(ID::B)), y.size());
				auto s_let_name = get_expression_for_signal(sigmap(cell->getPort(ID::S)), -1);
				auto y_let_name = get_expression_for_signal(y, -1);

				f << stringf("(union %s (Op3 (Mux) %s %s %s))\n", y_let_name.c_str(), s_let_name.c_str(), a_let_name.c_str(),
										 b_let_name.c_str())
								 .c_str();
			}
			else if (cell->type.in(ID($eq), ID($logic_and), ID($logic_or), ID($ne)))
			{
				// Binary ops that result in one bit.
				assert(cell->connections().size() == 3);
				auto y = sigmap(cell->getPort(ID::Y));
				auto a = sigmap(cell->getPort(ID::A));
				auto b = sigmap(cell->getPort(ID::B));

				if (y.size() != 1)
					log_error("Expected 1-bit output for cell %s.\n", log_id(cell));

				// Extend the inputs to the same width.
				// TODO(@gussmith23): there might be Yosys pass to do this
				// automatically; see:
				// https://github.com/YosysHQ/yosys/discussions/4368
				int to_width = std::max(a.size(), b.size());
				auto a_let_name = get_expression_for_signal(sigmap(cell->getPort(ID::A)), to_width);
				auto b_let_name = get_expression_for_signal(sigmap(cell->getPort(ID::B)), to_width);

				auto y_let_name = get_expression_for_signal(y, -1);

				std::string op_str;
				if (cell->type == ID($eq))
					op_str = "(Eq)";
				else if (cell->type == ID($logic_and))
					op_str = "(LogicAnd)";
				else if (cell->type == ID($logic_or))
					op_str = "(LogicOr)";
				else if (cell->type == ID($ne))
					op_str = "(Ne)";
				else
					log_error("This should be unreachable. You are missing an else if branch.\n");

				f << stringf("(union %s (Op2 %s %s %s))\n", y_let_name.c_str(), op_str.c_str(), a_let_name.c_str(),
										 b_let_name.c_str())
								 .c_str();
			}
			else if (cell->type == ID($dff))
			{
				assert(cell->connections().size() == 3);
				auto q = sigmap(cell->getPort(ID::Q));
				auto d_let_name = get_expression_for_signal(sigmap(cell->getPort(ID::D)), q.size());
				auto clk_let_name = get_expression_for_signal(sigmap(cell->getPort(ID::CLK)), -1);
				auto q_let_name = get_expression_for_signal(q, -1);

				f << "; TODO: assuming 0 default for Reg\n";
				f << stringf("(union %s (Op2 (Reg 0) %s %s))\n", q_let_name.c_str(), clk_let_name.c_str(), d_let_name.c_str());
			}
			else if (cell->type == ID($pmux))
			{
				// Don't support $pmux: require them to run pmuxtree instead.
				log_error("Unsupported cell type %s for cell %s.%s -- please run `pmuxtree` before `write_churchroad`.\n",
									log_id(cell->type), log_id(module), log_id(cell));
			}
			else if (cell->has_attribute("\\src"))
			{
				// Instance of a user-defined module.
				// TODO(@gussmith23): is this the best way to determine whether it's a
				// user-defined module? By this I mean, not an inbuilt primitive to
				// Yosys.

				std::vector<std::pair<std::string, std::string>> input_port_names_and_exprs;
				std::vector<std::pair<std::string, std::string>> output_port_names_and_exprs;
				std::vector<std::pair<std::string, std::string>> input_parameter_names_and_exprs;

				for (auto connection : cell->connections())
				{
					auto port = connection.first;
					auto port_name = port.str();
					auto sig = connection.second;
					auto sig_let_name = get_expression_for_signal(sig, -1);

					assert(cell->input(port) || cell->output(port));

					if (cell->input(port))
					{
						input_port_names_and_exprs.push_back({port_name, sig_let_name});
					}
					else if (cell->output(port))
					{
						output_port_names_and_exprs.push_back({port_name, sig_let_name});
					}
				}

				auto compile_const = [](RTLIL::Const c)
				{
					if ((c.flags & RTLIL::ConstFlags::CONST_FLAG_STRING) == RTLIL::ConstFlags::CONST_FLAG_STRING)
					{
						return stringf("(Op0 (CRString \"%s\"))", c.decode_string().c_str());
					}
					else
					{
						return stringf("(Op0 (BV %d %zu))", c.as_int(false), c.bits.size());
					}
				};
				for (auto parameter : cell->parameters)
				{

					input_parameter_names_and_exprs.push_back({parameter.first.c_str(),
																										 compile_const(parameter.second)});
				}

				// Generate the instance.
				// Cut the "\" off the front.
				// Check that it starts with "\" first, though.
				assert(cell->type[0] == '\\');
				assert(cell->name[0] == '\\');
				f << stringf("(let %s (ModuleInstance \"%s\" ", cell->name.substr(1).c_str(), cell->type.substr(1).c_str()).c_str();
				std::string closer = "(StringNil)";
				for (auto [parameter_name, _] : input_parameter_names_and_exprs)
				{
					assert(parameter_name[0] == '\\');
					f << stringf("(StringCons \"%s\"", parameter_name.substr(1).c_str()).c_str();
					closer.append(")");
				}
				f << closer;
				closer = "(ExprNil)";
				for (auto [_, expr] : input_parameter_names_and_exprs)
				{
					f << stringf("(ExprCons %s", expr.c_str()).c_str();
					closer.append(")");
				}
				f << closer;
				closer = "(StringNil)";
				for (auto [port_name, _] : input_port_names_and_exprs)
				{
					assert(port_name[0] == '\\');
					f << stringf("(StringCons \"%s\"", port_name.substr(1).c_str()).c_str();
					closer.append(")");
				}
				f << closer;
				closer = "(ExprNil)";
				for (auto [_, expr] : input_port_names_and_exprs)
				{
					f << stringf("(ExprCons %s", expr.c_str()).c_str();
					closer.append(")");
				}
				f << closer;
				f << "))\n";

				// Hook up the outputs.
				for (auto [port_name, expr] : output_port_names_and_exprs)
				{
					assert(port_name[0] == '\\');
					assert(cell->name[0] == '\\');
					f << stringf("(union (GetOutput %s \"%s\") %s)\n", cell->name.substr(1).c_str(), port_name.substr(1).c_str(), expr.c_str()).c_str();
				}
			}
			else
			{
				log_error("Unimplemented cell type %s for cell %s.%s.\n", log_id(cell->type), log_id(module), log_id(cell));
			}
		}

		// For each input, generate Var expression and mark it as an input port
		// using the IsPort relation. Also, union it with the corresponding wire.
		f << "\n; inputs\n";
		for (auto wire : module->wires())
		{
			if (!wire->port_id || !wire->port_input)
				continue;

			auto sigspec = sigmap(wire);
			auto signal_name = get_signal_name(sigspec);

			assert(signal_let_bound_name.count(sigspec));
			auto let_bound_id = signal_let_bound_name.at(sigspec);

			f << stringf("(IsPort \"%s\" \"%s\" (Input) %s)\n", /*module name*/ "", signal_name.c_str(), let_bound_id.c_str()).c_str();
			if (let_bindings)
			{
				f << stringf("(let %s (Var \"%s\" %d))\n", signal_name.c_str(), signal_name.c_str(), GetSize(sigspec)).c_str();
				f << stringf("(union %s %s)\n", let_bound_id.c_str(), signal_name.c_str()).c_str();
			}
			// If port name is in port_to_expr_map, union it with the expression.
			if (port_to_expr_map.count(signal_name))
			{
				f << stringf("(union %s %s)\n", let_bound_id.c_str(), port_to_expr_map.at(signal_name).c_str()).c_str();
			}
		}

		// For each output, mark it as an output port using the IsPort relation.
		f << "\n; outputs\n";
		for (auto wire : module->wires())
		{
			if (!wire->port_id || !wire->port_output)
				continue;

			// The name before sigmapping should be the output port name, i hope!
			auto signal_name_pre_sigmap = get_signal_name(wire);

			// The name after sigmapping will help us figure out which let ID holds the value of this signal.
			auto sigspec = sigmap(wire);
			auto signal_name = get_signal_name(sigspec);
			assert(signal_let_bound_name.count(sigspec));
			auto let_bound_id = signal_let_bound_name.at(sigspec);

			f << stringf("(IsPort \"%s\" \"%s\" (Output) %s)\n", /*module name*/ "", signal_name_pre_sigmap.c_str(), let_bound_id.c_str()).c_str();
			if (let_bindings)
			{
				f << stringf("(let %s %s)\n", signal_name_pre_sigmap.c_str(), let_bound_id.c_str()).c_str();
			}
			// If port name is in port_to_expr_map, union it with the expression.
			if (port_to_expr_map.count(signal_name_pre_sigmap))
			{
				f << stringf("(union %s %s)\n", let_bound_id.c_str(), port_to_expr_map.at(signal_name_pre_sigmap).c_str()).c_str();
			}
		}

		// Run typing rules before deleting wires -- cyclic circuits can only be typed using Wire expresions to bootstrap the types.
		f << "\n; run typing rules\n";
		f << "(run-schedule (saturate typing))\n";

		// Delete Wire expressions.
		f << "\n; delete wire expressions\n";
		f << "\n; TODO(@gussmith23): I'm not actually sure we want to delete wires. Sometimes there's nothing attached to a wire, e.g. when it's used to store the unused output of a module. We don't want to delete it in that case!\n";
		// for (auto wire_expr : wire_exprs)
		// {
		// 	f << stringf("(delete %s)\n", wire_expr.c_str()).c_str();
		// }
	}
};

struct ChurchroadBackend : public Backend
{
	ChurchroadBackend() : Backend("churchroad", "write design to egglog Churchroad IR") {}
	void help() override
	{
		//   |---v---|---v---|---v---|---v---|---v---|---v---|---v---|---v---|---v---|---v---|
		log("\n");
		log("    write_churchroad [options] [filename]\n");
		log("\n");
		log("Write a Churchroad description of the current design.\n");
		log("\n");
		log("  -salt\n");
		log("    Salt string to add to all egglog commands.\n");
		// log("\n");
		// log("  -s\n");
		// log("    Output only a single bad property for all asserts\n");
		// log("\n");
		// log("  -c\n");
		// log("    Output cover properties using 'bad' statements instead of asserts\n");
		// log("\n");
		// log("  -i <filename>\n");
		// log("    Create additional info file with auxiliary information\n");
		// log("\n");
		// log("  -x\n");
		// log("    Output symbols for internal netnames (starting with '$')\n");
		// log("\n");
		// log("  -ywmap <filename>\n");
		// log("    Create a map file for conversion to and from Yosys witness traces\n");
		// log("\n");
	}
	void execute(std::ostream *&f, std::string filename, std::vector<std::string> args, RTLIL::Design *design) override
	{
		log_header(design, "Executing Lakeroad egglog backend.\n");

		// These passes put the design in a form that is convenient for Churchroad
		// conversion. Specifically, "piecewise" assignments like the following:
		// ```
		// assign sig[0] = ...;
		// assign sig[1] = ...;
		// assign sig[2:...] = ...;
		// ```
		// are converted to a single assignment like the following:
		// ```
		// assign sig = ...;
		// ```
		// This is necessary to prevent the bug described here:
		// https://github.com/uwsampl/churchroad/issues/13
		//
		// The `clean` pass simply removes any unused wires, see this discussion:
		// https://github.com/YosysHQ/yosys/discussions/4299
		Pass::call(design, "splice");
		Pass::call(design, "splitnets -driver");
		Pass::call(design, "clean");

		RTLIL::Module *topmod = design->top_module();

		// Maps port names to egglog expression strings that they should be `union`ed with.
		std::map<std::string, std::string> port_to_expr_map;
		std::string salt = "";
		bool let_bindings = false;
		for (size_t arg_i = 1; arg_i < args.size();)
		{
			// Look for -portunion option
			if (args[arg_i] == "-portunion")
			{
				log_assert(args.size() > arg_i + 2);
				// Replace underscores with spaces.
				// TODO(@gussmith23): Shouldn't be needed after we figure out quoting in Yosys commands.
				port_to_expr_map[args[arg_i + 1]] =
						std::regex_replace(args[arg_i + 2], std::regex("_"), " ");
				arg_i += 3;
				continue;
			}

			// Look for -salt option
			if (args[arg_i] == "-salt")
			{
				log_assert(args.size() > arg_i + 1);
				salt = args[arg_i + 1];
				arg_i += 2;
				continue;
			}

			// Look for filename as last argument.
			if (arg_i == args.size() - 1 && args[arg_i][0] != '-')
			{
				// Confusingly, this is failing when filename is ""
				// log_assert(filename.empty());
				filename = args[arg_i];
				arg_i++;
				continue;
			}

			// Look for -letbindings option
			if (args[arg_i] == "-letbindings")
			{
				let_bindings = true;
				arg_i += 1;
				continue;
			}

			log_error("Unrecognized option: %s", args[arg_i].c_str());
		}

		// Has to come after other arg parsing.
		extra_args(f, filename, args, args.size());

		if (topmod == nullptr)
			log_cmd_error("No top module found.\n");

		LakeroadWorker(*f, topmod, salt, let_bindings, port_to_expr_map).run();

		// *f << stringf("; end of yosys output\n");
	}
} ChurchroadBackend;

PRIVATE_NAMESPACE_END
