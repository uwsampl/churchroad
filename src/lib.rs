use egraph_serialize::{ClassId, Node};
use indexmap::IndexMap;
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use egglog::{
    ast::{Literal, Symbol},
    constraint::{SimpleTypeConstraint, TypeConstraint},
    sort::{FromSort, I64Sort, IntoSort, Sort, VecSort},
    ArcSort, EGraph, PrimitiveLike, Term, TermDag, Value,
};

// The result of interpreting a Churchroad program.
#[derive(Debug, PartialEq, Clone)]
pub enum InterpreterResult {
    // Bitvector(value, bitwidth)
    Bitvector(u64, u64),
}

/// Interprets a Churchroad program.
///
/// ```
/// use churchroad::*;
/// use egglog::{EGraph, SerializeConfig};
/// use egraph_serialize::NodeId;
/// let mut egraph = EGraph::default();
/// import_churchroad(&mut egraph);
/// egraph.parse_and_run_program(
/// r#"
/// (let v0 (Wire "v0" 1))
/// (let v1 (Wire "v1" 1))
/// (let v2 (Wire "v2" 1))
/// (union v2 (Op2 (And) v0 v1))
/// (let a (Var "a" 1))
/// (IsPort "" "a" (Input) a)
/// (union v0 a)
/// (let b (Var "b" 1))
/// (IsPort "" "b" (Input) b)
/// (union v1 b)
/// (let out v2)
/// (IsPort "" "out" (Output) out)
/// (delete (Wire "v0" 1))
/// (delete (Wire "v1" 1))
/// (delete (Wire "v2" 1))
/// "#
/// ).unwrap();
///
/// let serialized = egraph.serialize(SerializeConfig::default());
///
/// // now, let's get the class ID of the output node
/// let (_, is_output_node) = serialized
///     .nodes
///     .iter()
///     .find(|(_, n)| n.op == "IsPort" && n.children[2] == NodeId::from("Output-0"))
///     .unwrap();
/// let output_id = is_output_node.children.last().unwrap();
/// let (_, output_node) = serialized
///     .nodes
///     .iter()
///     .find(|(node_id, _)| **node_id == *output_id)
///     .unwrap();
///
/// let result = interpret(&serialized, &output_node.eclass, 0,
///     &[("a", vec![1]), ("b", vec![1])].into()
/// );
///
/// assert_eq!(result, Ok(InterpreterResult::Bitvector(1, 1)));
///
/// ```
pub fn interpret(
    egraph: &egraph_serialize::EGraph,
    class_id: &ClassId,
    time: usize,
    env: &HashMap<&str, Vec<u64>>,
) -> Result<InterpreterResult, String> {
    let result = match egraph.classes().iter().find(|(id, _)| *id == class_id) {
        Some((id, _)) => interpret_helper(egraph, id, time, env),
        None => return Err("No class with the given ID.".to_string()),
    };

    result
}

fn interpret_helper(
    egraph: &egraph_serialize::EGraph,
    id: &ClassId,
    time: usize,
    env: &HashMap<&str, Vec<u64>>,
) -> Result<InterpreterResult, String> {
    let node_ids = &egraph.classes().get(id).unwrap().nodes;
    if node_ids.len() != 1 {
        return Err(format!(
            "There should be exactly one node in the class, but there are {}.",
            node_ids.len()
        ));
    }

    let node_id = node_ids.first().unwrap();
    let node = egraph.nodes.get(node_id).unwrap();

    match node.op.as_str() {
        "Var" => {
            let bw: u64 = egraph
                .nodes
                .get(&node.children[1])
                .unwrap()
                .op
                .parse()
                .unwrap();
            let name = egraph.nodes.get(&node.children[0]).unwrap().op.as_str();
            // cut off the quotes on the beginning and end
            let name = &name[1..name.len() - 1];

            Ok(InterpreterResult::Bitvector(
                *env.get(name)
                    .unwrap_or_else(|| panic!("didn't find var {:?}", name))
                    .get(time)
                    .unwrap(),
                bw,
            ))
        }
        "Op0" | "Op1" | "Op2" | "Op3" => {
            assert!(!node.children.is_empty());
            let op = egraph.nodes.get(&node.children[0]).unwrap();

            let children: Vec<_> = node
                .children
                .iter()
                .skip(1)
                .map(|id| {
                    let child = egraph.nodes.get(id).unwrap();
                    interpret_helper(egraph, &child.eclass, time, env)
                })
                .collect();

            match op.op.as_str() {
                // Binary operations that preserve bitwidth.
                "And" | "Or" | "Shr" => {
                    assert_eq!(children.len(), 2);
                    match (&children[0], &children[1]) {
                        (
                            Ok(InterpreterResult::Bitvector(a, a_bw)),
                            Ok(InterpreterResult::Bitvector(b, b_bw)),
                        ) => {
                            assert_eq!(a_bw, b_bw);
                            let result = match op.op.as_str() {
                                "And" => a & b,
                                "Or" => a | b,
                                "Shr" => a >> b,
                                _ => unreachable!(),
                            };
                            Ok(InterpreterResult::Bitvector(result, *a_bw))
                        }
                        _ => todo!(),
                    }
                }
                "Mux" => {
                    assert_eq!(children.len(), 3);

                    match children[0] {
                        Ok(InterpreterResult::Bitvector(cond, _)) => {
                            if cond == 0 {
                                children[1].clone()
                            } else {
                                children[2].clone()
                            }
                        }
                        _ => todo!(),
                    }
                }
                "BV" => {
                    assert_eq!(op.children.len(), 2);
                    let args = &op
                        .children
                        .iter()
                        .map(|id| {
                            let (_, node) = egraph
                                .nodes
                                .iter()
                                .find(|(node_id, _)| **node_id == *id)
                                .unwrap();
                            assert_eq!(node.children.len(), 0);
                            let val: u64 = node.op.parse().unwrap();
                            val
                        })
                        .collect::<Vec<_>>()[..];

                    assert!(args[1] <= 64);
                    Ok(InterpreterResult::Bitvector(args[0], args[1]))
                }
                "Extract" => {
                    assert_eq!(op.children.len(), 2);
                    let args = &op
                        .children
                        .iter()
                        .map(|id| {
                            let (_, node) = egraph
                                .nodes
                                .iter()
                                .find(|(node_id, _)| **node_id == *id)
                                .unwrap();
                            assert_eq!(node.children.len(), 0);
                            let val: u64 = node.op.parse().unwrap();
                            val
                        })
                        .collect::<Vec<_>>()[..];

                    let i = args[0];
                    let j = args[1];

                    let val = match children[0].as_ref().unwrap() {
                        InterpreterResult::Bitvector(val, bw) => {
                            // from Rosette docs:
                            // https://docs.racket-lang.org/rosette-guide/sec_bitvectors.html#%28def._%28%28lib._rosette%2Fbase%2Fbase..rkt%29._extract%29%29
                            // TODO(@ninehusky): here, we should also assert that j >= 0 if churchroad handles signed numbers
                            assert!(*bw > i && i >= j);

                            let mask = (1 << (i - j + 1)) - 1;
                            (val >> j) & mask
                        }
                    };
                    assert!(i - j < 64);
                    Ok(InterpreterResult::Bitvector(val, i - j + 1))
                }
                "Concat" => match (&children[0], &children[1]) {
                    (
                        Ok(InterpreterResult::Bitvector(a, a_bw)),
                        Ok(InterpreterResult::Bitvector(b, b_bw)),
                    ) => {
                        let result = (a << b_bw) | b;
                        assert!(a_bw + b_bw <= 64);
                        Ok(InterpreterResult::Bitvector(result, a_bw + b_bw))
                    }
                    _ => todo!(),
                },
                "ZeroExtend" => {
                    let extension_bw: u64 = egraph
                        .nodes
                        .iter()
                        .find(|(id, _)| *id == &op.children[0])
                        .unwrap()
                        .1
                        .op
                        .parse()
                        .unwrap();
                    assert!(extension_bw <= 64);
                    match children[0] {
                        Ok(InterpreterResult::Bitvector(val, _)) => {
                            Ok(InterpreterResult::Bitvector(val, extension_bw))
                        }
                        _ => todo!(),
                    }
                }
                _ => todo!("unimplemented op: {:?}", op.op),
            }
        }
        _ => todo!("unimplemented node type: {:?}", node.op),
    }
}

#[derive(Default)]
pub struct AnythingExtractor;
impl AnythingExtractor {
    pub fn extract(
        &self,
        egraph: &egraph_serialize::EGraph,
        _roots: &[egraph_serialize::ClassId],
    ) -> IndexMap<egraph_serialize::ClassId, egraph_serialize::NodeId> {
        egraph
            .classes()
            .iter()
            .map(|(id, class)| {
                let node_id = class.nodes.first().unwrap().clone();
                (id.clone(), node_id)
            })
            .collect()
    }
}

pub fn to_verilog_egraph_serialize(
    egraph: &egraph_serialize::EGraph,
    choices: &IndexMap<egraph_serialize::ClassId, egraph_serialize::NodeId>,
    clk_name: &str,
) -> String {
    // let mut wires = HashMap::default();

    fn id_to_wire_name(id: &ClassId) -> String {
        format!("wire_{}", id)
    }

    struct ModuleInstance {
        module_class_name: String,
        instance_name: String,
        parameters: HashMap<String, ClassId>,
        inputs: HashMap<String, ClassId>,
        outputs: HashMap<String, ClassId>,
    }
    // Maps EClass ID to the module instance at that class.
    let mut module_instantiations: HashMap<ClassId, ModuleInstance> = HashMap::new();

    let mut inputs = String::new();
    let mut outputs = String::new();
    let mut logic_declarations = String::new();
    let mut registers = String::new();

    // Collect all the outputs.
    let mut queue: Vec<ClassId> = egraph
        .nodes
        .iter()
        .filter_map(|(_id, node)| {
            // op should be IsPort
            let op = &node.op;
            if op != "IsPort" {
                return None;
            }

            assert_eq!(node.children.len(), 4);

            if egraph[&node.children[2]].op != "Output" {
                return None;
            }

            Some(egraph[&node.children[3]].eclass.clone())
        })
        .collect();

    // Generate outputs.
    for (_, node) in egraph.nodes.iter() {
        // op should be IsPort
        let op = &node.op;
        if op != "IsPort" {
            continue;
        }

        assert_eq!(node.children.len(), 4);

        if egraph[&node.children[2]].op != "Output" {
            continue;
        }

        outputs.push_str(&format!(
            "output {name},\n",
            name = egraph[&node.children[1]]
                .op
                .as_str()
                .strip_prefix('\"')
                .unwrap()
                .strip_suffix('\"')
                .unwrap()
        ));

        logic_declarations.push_str(&format!(
            "logic {name} = {wire};\n",
            name = egraph[&node.children[1]]
                .op
                .as_str()
                .strip_prefix('\"')
                .unwrap()
                .strip_suffix('\"')
                .unwrap(),
            wire = id_to_wire_name(&egraph[&node.children[3]].eclass)
        ))
    }

    let mut done = HashSet::new();

    fn maybe_push_expr_on_queue(
        queue: &mut Vec<ClassId>,
        done: &HashSet<ClassId>,
        class_id: &ClassId,
    ) {
        if !queue.contains(class_id) && !done.contains(class_id) {
            queue.push(class_id.clone());
        }
    }

    while let Some(id) = queue.pop() {
        done.insert(id.clone());
        let term = &egraph[&choices[&id]];

        let op = &term.op;
        match op.as_str() {
            // Things to ignore.
            //
            // Ignore the Unit.
            "()" |
            // Ignore various relations/facts.
            "IsPort" |
            "Input" |
            "Output" |
            // Ignore the nodes for the ops themselves.
            "ZeroExtend" |
            "Concat" |
            "Extract" |
            "Or" |
            "And" |
            "Add" |
            "Shr" |
            "Eq" |
            "Xor" |
            "Reg" => (),
            // Ignore integer literals.
            v if v.parse::<i64>().is_ok() => (),

            "Op0" | "Op1" | "Op2" => {
                let op_node = &egraph[&term.children[0]];
                match op_node.op.as_str() {
                    "ZeroExtend" => {
                        assert_eq!(op_node.children.len(), 1);
                        assert_eq!(term.children.len(), 2);
                        let bw = egraph[&op_node.children[0]].op.parse::<i64>().unwrap();
                    logic_declarations.push_str(
                        format!(
                            "logic [{bw}-1:0] {this_wire} = {bw}'d{value};\n",
                            this_wire = id_to_wire_name(&id),
                            value = id_to_wire_name(&egraph[&term.children[1]].eclass)

                        )
                        .as_str(),
                    );

                    }
                    "BV" => {
                        assert_eq!(op_node.children.len(), 2);
                        let value = egraph[&op_node.children[0]].op.parse::<i64>().unwrap();
                        let bw = egraph[&op_node.children[1]].op.parse::<i64>().unwrap();

                    logic_declarations.push_str(
                        format!(
                            "logic [{bw}-1:0] {this_wire} = {bw}'d{value};\n",
                            this_wire = id_to_wire_name(&id),
                        )
                        .as_str(),
                    );
                    }
                    "Reg" => {
                        let default_val = egraph[&op_node.children[0]].op.parse::<i64>().unwrap();
                        let d_id = &egraph[&term.children[1]].eclass;


                    logic_declarations.push_str(
                        format!(
                            "logic {this_wire} = {default};\n",
                            this_wire = id_to_wire_name(&id),
                            default = default_val
                        )
                        .as_str(),
                    );

                    registers.push_str(&format!(
                        "always @(posedge {clk_name}) begin
                            {this_wire} <= {d};
                        end\n",
                        // clk = id_to_wire_name(clk_id),
                        this_wire = id_to_wire_name(&id),
                        d = id_to_wire_name(d_id)
                    ));

                    if !done.contains(d_id) {
                        queue.push(d_id.clone());
                    }
                    },
                    "Concat" | "Xor" |"And" | "Or" =>  {
                            assert_eq!(term.children.len(), 3);
                    let expr0_id = &egraph[&term.children[1]].eclass;
                    let  expr1_id = &egraph[&term.children[2]].eclass;
                    logic_declarations.push_str(&format!(
                        "logic {this_wire} = {op};\n",
                        op = match op_node.op.as_str() {

                            "Concat" => format!("{{ {expr0}, {expr1} }}",
                        expr0 = id_to_wire_name(expr0_id),
                        expr1 = id_to_wire_name(expr1_id),
                        ),
                            "Xor" => format!("{expr0}^{expr1}",
                        expr0 = id_to_wire_name(expr0_id),
                        expr1 = id_to_wire_name(expr1_id),
                        ),
                            "And" => format!("{expr0}&{expr1}",
                        expr0 = id_to_wire_name(expr0_id),
                        expr1 = id_to_wire_name(expr1_id),
                        ),
                            "Or" => format!("{expr0}|{expr1}",
                        expr0 = id_to_wire_name(expr0_id),
                        expr1 = id_to_wire_name(expr1_id),
                        ),
                        _ => unreachable!("missing a match arm"),
                        } ,
                        this_wire = id_to_wire_name(&term.eclass),
                    ));

                    maybe_push_expr_on_queue(&mut queue, &done, expr0_id);
                    maybe_push_expr_on_queue(&mut queue, &done, expr1_id);
                }
                "Extract" => {//}, [hi_id, lo_id, expr_id]) => {
                    assert_eq!(term.children.len(), 2);
                    assert_eq!(op_node.children.len(), 2);
                    let hi:i64 = egraph[&op_node.children[0]].op.parse().unwrap();
                    let lo:i64 = egraph[&op_node.children[1]].op.parse().unwrap();
                    let id = &term.eclass;
                    let expr_id = &egraph[&term.children[1]].eclass;
                    logic_declarations.push_str(&format!(
                        "logic {this_wire} = {expr}[{hi}:{lo}];\n",
                        hi = hi,
                        lo = lo,
                        this_wire = id_to_wire_name(id),
                        expr = id_to_wire_name(expr_id),
                    ));

                    maybe_push_expr_on_queue(&mut queue, &done, expr_id);
                }

                v => todo!("{:?}", v),

                }

            }

                "Var" => {//}, [name_id, bw_id]) => {
                    assert_eq!(term.children.len(), 2);

                        let name = egraph[&term.children[0]].op.as_str().strip_prefix('\"').unwrap().strip_suffix('\"').unwrap();
                        let bw: i64 = egraph[&term.children[1]].op.parse().unwrap();

                    inputs.push_str(
                        format!("input [{bw}-1:0] {name},\n", bw = bw, name = name).as_str(),
                    );

                    logic_declarations.push_str(
                        format!(
                            "logic [{bw}-1:0] {this_wire} = {name};\n",
                            bw = bw,
                            this_wire = id_to_wire_name(&term.eclass),
                            name = name
                        )
                        .as_str(),
                    );
                }

                // Skip string literals.
            _ if term.eclass.to_string().starts_with("String") => (),

            "GetOutput" => {
                assert_eq!(term.children.len(), 2);

                let module_class = &egraph[&term.children[0]].eclass;
                let _output_class = &egraph[&term.children[1]].eclass;
                let output_name = egraph[&term.children[1]].op.as_str().strip_prefix('\"').unwrap().strip_suffix('\"').unwrap();

                // get module class name (e.g. mymodule in `mymodule m (ports);`)
                assert_eq!(egraph[module_class].nodes.len(),1);
                let module_instance_node = &egraph[&egraph[module_class].nodes[0]];
                assert_eq!(module_instance_node.op, "ModuleInstance");
                assert_eq!(module_instance_node.children.len(), 5);
                let module_class_name = egraph[&module_instance_node.children[0].clone()].op.as_str().strip_prefix('\"').unwrap().strip_suffix('\"').unwrap();


                fn cons_list_to_vec(egraph: &egraph_serialize::EGraph, cons_class_id: &ClassId) -> Vec<ClassId> {
                    assert_eq!(egraph[cons_class_id].nodes.len(), 1);
                    let cons_node = &egraph[&egraph[cons_class_id].nodes[0]];
                    match cons_node.op.as_str() {
                        "StringCons" | "ExprCons" => {
                            assert_eq!(cons_node.children.len(), 2);
                            [egraph[&cons_node.children[0]].eclass.clone()].iter().chain(cons_list_to_vec(egraph, &egraph[&cons_node.children[1]].eclass).iter()).cloned().collect()
                        }
                        "StringNil" | "ExprNil" => {
                            assert_eq!(cons_node.children.len(), 0);
                            vec![]
                        }
                        _ => unreachable!()
                    }

                }

                fn class_id_vec_to_strings(egraph: &egraph_serialize::EGraph, class_id_vec: Vec<ClassId>) -> Vec<String> {
                    class_id_vec.iter().map(|id| {
                        assert_eq!(egraph[id].nodes.len(), 1);
                        egraph[&egraph[id].nodes[0]].op.as_str().strip_prefix('\"').unwrap().strip_suffix('\"').unwrap().to_owned()
                    }).collect()
                }

                // Get module input names and input exprs.
                let parameter_names= class_id_vec_to_strings(egraph, cons_list_to_vec(egraph, &egraph[&module_instance_node.children[1]].eclass));
                let parameter_exprs=  cons_list_to_vec(egraph, &egraph[&module_instance_node.children[2]].eclass);
                let input_port_names= class_id_vec_to_strings(egraph, cons_list_to_vec(egraph, &egraph[&module_instance_node.children[3]].eclass));
                let input_port_exprs=  cons_list_to_vec(egraph, &egraph[&module_instance_node.children[4]].eclass);
                assert_eq!(parameter_exprs.len(), parameter_names.len());
                assert_eq!(input_port_exprs.len(), input_port_names.len());

                for expr in input_port_exprs.iter().chain(parameter_exprs.iter()) {
                    maybe_push_expr_on_queue(&mut queue, &done, expr);
                }

                // If we haven't seen this module yet, create a new module instance.
                if !module_instantiations.contains_key(module_class) {
                    module_instantiations.insert(module_class.clone(), ModuleInstance {
                        module_class_name: module_class_name.to_owned(),
                        instance_name: format!("module_{}", module_class),
                        parameters: parameter_names.into_iter().zip(parameter_exprs.into_iter()).collect(),
                        inputs: input_port_names.into_iter().zip(input_port_exprs.into_iter()).collect(),
                        outputs: [(output_name.to_owned(), term.eclass.clone())].into(),
                    });
                } else if let Some(module_instance) = module_instantiations.get_mut(module_class) {
                    module_instance.outputs.insert(output_name.to_owned(), term.eclass.clone());
                }else {
                    unreachable!("module_instantiations should contain the module class");
                }

                logic_declarations.push_str(
                    format!(
                        "logic {this_wire};\n",
                        this_wire = id_to_wire_name(&term.eclass),
                    )
                    .as_str(),
                );
            }

            // Term::Lit(Literal::Int(v)) => {
            //     logic_declarations.push_str(&format!(
            //         "logic [31:0] {this_wire} = {val};\n",
            //         this_wire = id_to_wire_name(id),
            //         val = v
            //     ));
            // }
            // Term::Var(_) => todo!(),
            // Term::App(s, v) => match (s.as_str(), v.as_slice()) {
            //     ("Reg", &[default_id, clk_id, d_id]) => {
            //         let default_val = match term_dag.get(default_id) {
            //             Term::Lit(Literal::Int(default_val)) => default_val,
            //             _ => panic!(),
            //         };

            //         logic_declarations.push_str(
            //             format!(
            //                 "logic {this_wire} = {default};\n",
            //                 this_wire = id_to_wire_name(id),
            //                 default = default_val
            //             )
            //             .as_str(),
            //         );

            //         registers.push_str(&format!(
            //             "always @(posedge {clk}) begin
            //                 {this_wire} <= {d};
            //             end\n",
            //             clk = id_to_wire_name(clk_id),
            //             this_wire = id_to_wire_name(id),
            //             d = id_to_wire_name(d_id)
            //         ));

            //         if !done.contains(&d_id) {
            //             queue.push(d_id);
            //         }
            //         if !done.contains(&clk_id) {
            //             queue.push(clk_id);
            //         }
            //     }
            //     ("Var", [name_id, bw_id]) => {
            //         let name = match term_dag.get(*name_id) {
            //             Term::Lit(Literal::String(name)) => name,
            //             _ => panic!(),
            //         };
            //         let bw = match term_dag.get(*bw_id) {
            //             Term::Lit(Literal::Int(bw)) => bw,
            //             _ => panic!(),
            //         };

            //         inputs.push_str(
            //             format!("input [{bw}-1:0] {name};\n", bw = bw, name = name).as_str(),
            //         );

            //         logic_declarations.push_str(
            //             format!(
            //                 "logic [{bw}-1:0] {this_wire} = {name};\n",
            //                 bw = bw,
            //                 this_wire = id_to_wire_name(id),
            //                 name = name
            //             )
            //             .as_str(),
            //         );
            //     }
            //     ("Mux", []) => (),
            //     ("LUT4", []) => (),
            //     ("Or", []) => (),
            //     ("Bitvector", [_]) => (),
            //     ("Eq", []) => (),
            //     ("BV", [val_id, bw_id]) => {
            //         let val = match term_dag.get(*val_id) {
            //             Term::Lit(Literal::Int(val)) => val,
            //             _ => panic!(),
            //         };
            //         let bw = match term_dag.get(*bw_id) {
            //             Term::Lit(Literal::Int(bw)) => bw,
            //             _ => panic!(),
            //         };
            //         logic_declarations.push_str(
            //             format!(
            //                 "logic [{bw}-1:0] {this_wire} = {bw}'d{val};\n",
            //                 bw = bw,
            //                 this_wire = id_to_wire_name(id),
            //                 val = val
            //             )
            //             .as_str(),
            //         );
            //     }
            //     ("Extract", [hi_id, lo_id, expr_id]) => {
            //         let hi = match term_dag.get(*hi_id) {
            //             Term::Lit(Literal::Int(hi)) => hi,
            //             _ => panic!(),
            //         };
            //         let lo = match term_dag.get(*lo_id) {
            //             Term::Lit(Literal::Int(lo)) => lo,
            //             _ => panic!(),
            //         };
            //         logic_declarations.push_str(&format!(
            //             "logic {this_wire} = {expr}[{hi}:{lo}];\n",
            //             hi = hi,
            //             lo = lo,
            //             this_wire = id_to_wire_name(id),
            //             expr = id_to_wire_name(*expr_id),
            //         ));

            //         if !done.contains(&expr_id) {
            //             queue.push(*expr_id);
            //         }
            //     }
            //     ("Concat", [expr0_id, expr1_id]) => {
            //         logic_declarations.push_str(&format!(
            //             "logic {this_wire} = {{ {expr0}, {expr1} }};\n",
            //             this_wire = id_to_wire_name(id),
            //             expr0 = id_to_wire_name(*expr0_id),
            //             expr1 = id_to_wire_name(*expr1_id),
            //         ));

            //         if !done.contains(&expr0_id) {
            //             queue.push(*expr0_id);
            //         }
            //         if !done.contains(&expr1_id) {
            //             queue.push(*expr1_id);
            //         }
            //     }
            //     ("ZeroExtend", [expr_id, bw_id]) => {
            //         let bw = match term_dag.get(*bw_id) {
            //             Term::Lit(Literal::Int(bw)) => bw,
            //             _ => panic!(),
            //         };
            //         logic_declarations.push_str(&format!(
            //             "logic {this_wire} = {{ {bw}'d0, {expr} }};\n",
            //             this_wire = id_to_wire_name(id),
            //             bw = bw,
            //             expr = id_to_wire_name(*expr_id),
            //         ));

            //         if !done.contains(&expr_id) {
            //             queue.push(*expr_id);
            //         }
            //     }
            //     ("Sketch1", [op_id, expr_id])
            //         if match term_dag.get(*op_id) {
            //             Term::App(s, v) => s.as_str() == "LUT4" && v.is_empty(),
            //             _ => false,
            //         } =>
            //     {
            //         logic_declarations.push_str(&format!(
            //             "logic {this_wire};\n",
            //             this_wire = id_to_wire_name(id),
            //         ));

            //         module_declarations.push_str(&format!(
            //             "lut4 lut4_{id} (.in({expr}), .out({y}));\n",
            //             id = id,
            //             expr = id_to_wire_name(*expr_id),
            //             y = id_to_wire_name(id),
            //         ));

            //         if !done.contains(&expr_id) {
            //             queue.push(*expr_id);
            //         }
            //     }
            //     _ => todo!("{:?}", (s, v)),
            // },
            _ => todo!("{:?}", &term),
        }
    }

    // For display purposes, we can clean this up later.
    // We sort to make the output stable.
    let inputs = {
        let mut out = inputs
            .split('\n')
            .map(|line| format!("  {}", line))
            .collect::<Vec<_>>();

        out.sort();
        out.join("\n")
    };
    let outputs = {
        let mut out = outputs
            .split('\n')
            .map(|line| format!("  {}", line))
            .collect::<Vec<_>>();
        out.sort();
        out.join("\n")
    };
    let logic_declarations = logic_declarations
        .split('\n')
        .map(|line| format!("  {}", line))
        .collect::<Vec<_>>()
        .join("\n");

    let module_instantiations = module_instantiations
        .iter()
        .map(
            |(
                _class_id,
                ModuleInstance {
                    module_class_name,
                    instance_name,
                    parameters,
                    inputs,
                    outputs,
                },
            )| {
                let parameters = parameters
                    .iter()
                    .map(|(name, id)| format!("    .{}({})", name, id_to_wire_name(id)))
                    .collect::<Vec<_>>()
                    .join(",\n");
                let inputs = {let mut out = inputs
                    .iter()
                    .map(|(name, id)| format!("    .{}({})", name, id_to_wire_name(id)))
                    .collect::<Vec<_>>();
                    out.sort();
                    out.join(",\n")};

                let outputs = {let mut out = outputs
                    .iter()
                    .map(|(name, id)| format!("    .{}({})", name, id_to_wire_name(id)))
                    .collect::<Vec<_>>();
                    out.sort();
                    out.join(",\n")};

                format!("  {module_class_name} #(\n{parameters}\n) {instance_name} (\n{inputs},\n{outputs});")
            },
        )
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        "module top(
{inputs}
{outputs}
);
{logic_declarations}
{registers}
{module_instantiations}
endmodule",
        inputs = inputs,
        logic_declarations = logic_declarations,
        registers = registers,
    )
}

pub fn to_rewrite_rule_egraph_serialize(
    egraph: &egraph_serialize::EGraph,
    choices: &IndexMap<egraph_serialize::ClassId, egraph_serialize::NodeId>,
    name: &str,
) -> String {
    let (inputs, outputs) = get_inputs_and_outputs_serialized(egraph);

    // Add all of the outputs to the queue
    let mut queue: Vec<ClassId> = outputs.into_iter().map(|c| c.1).collect();
    let mut done = HashSet::new();

    fn maybe_push_expr_on_queue(
        queue: &mut Vec<ClassId>,
        done: &HashSet<ClassId>,
        class_id: &ClassId,
    ) {
        if !queue.contains(class_id) && !done.contains(class_id) {
            queue.push(class_id.clone());
        }
    }

    let id_to_wire_name = |id: &ClassId| -> String {
        let v = inputs.iter().find(|(_k, v)| v == id);
        match v {
            Some((var, _id)) => var.to_string(),
            None => format!("wire_{}", id),
        }
    };

    let mut rules = String::new();

    // This is to deal with the generation of (Op2 (Extract b b) (Var x)) -> "x_b"
    // Keys are the variable names and values are list of which bits are extracted
    // This is used at the end to Concat all of the variables together.
    let mut input_extract_map: HashMap<String, Vec<i64>> = HashMap::new();

    while let Some(id) = queue.pop() {
        done.insert(id.clone());
        let term = &egraph[&choices[&id]];

        let op = &term.op;

        match op.as_str() {
            // Things to ignore.
            //
            // Ignore the Unit.
            "()" |
            // Ignore various relations/facts.
            "IsPort" |
            "Input" |
            "Output" |
            // Ignore the nodes for the ops themselves.
            "ZeroExtend" |
            "Concat" |
            "Extract" |
            "Or" |
            "And" |
            "Add" |
            "Shr" |
            "Eq" |
            "Xor" |
            "Reg" => (),
            // Ignore integer literals.
            v if v.parse::<i64>().is_ok() => (),

            "Op0" | "Op1" | "Op2" => {
                let op_node = &egraph[&term.children[0]];
                // BV, ZeroExtend, Add, Reg, working
                match op_node.op.as_str() {
                    "ZeroExtend" => {
                        assert_eq!(op_node.children.len(), 1);
                        assert_eq!(term.children.len(), 2);
                        let bw = egraph[&op_node.children[0]].op.parse::<i64>().unwrap();
                        let val_id = &egraph[&term.children[1]].eclass;
                        let o = format!(
                            "(= {this_wire} (Op1 (ZeroExtend {bw}) {value}))\n",
                            this_wire = id_to_wire_name(&id),
                            value = id_to_wire_name(val_id)
                            );
                        rules.push_str(o.as_str());

                        maybe_push_expr_on_queue(&mut queue, &done, val_id);
                    },

                    "Not" => {
                        assert_eq!(term.children.len(), 2);
                        let val_id = &egraph[&term.children[1]].eclass;
                        let o = format!(
                            "(= {this_wire} (Op1 (Not)  {value}))\n",
                            this_wire = id_to_wire_name(&id),
                            value = id_to_wire_name(val_id)
                            );
                        rules.push_str(o.as_str());
                        maybe_push_expr_on_queue(&mut queue, &done, val_id);

                    },
                    "BV" => {
                        assert_eq!(op_node.children.len(), 2);
                        let value = egraph[&op_node.children[0]].op.parse::<i64>().unwrap();
                        let bw = egraph[&op_node.children[1]].op.parse::<i64>().unwrap();
                        let o = format!(
                            "(= {this_wire} (Op0 (BV {value} {bw})))\n",
                            this_wire = id_to_wire_name(&id)
                            );
                        rules.push_str(o.as_str());
                    },
                    "Reg" => {
                        let default_val = egraph[&op_node.children[0]].op.parse::<i64>().unwrap();
                        let d_id = &egraph[&term.children[1]].eclass;
                        let val_id = &egraph[&term.children[2]].eclass;
                                                rules.push_str(
                        format!(
                                "(= {this_wire} (Op2 (Reg {default_val}) {val} {val1}))\n",
                                this_wire = id_to_wire_name(&id),
                                val = id_to_wire_name(d_id),
                                val1 = id_to_wire_name(val_id)
                                ).as_str()
                            );
                        if !done.contains(d_id) {
                            queue.push(d_id.clone());
                        }

                        maybe_push_expr_on_queue(&mut queue, &done, val_id);
                    },
                    "Concat" | "Xor" | "And" | "Or" | "Add" => {
                    let expr0_id = &egraph[&term.children[1]].eclass;
                    let  expr1_id = &egraph[&term.children[2]].eclass;

                    rules.push_str(
                        format!(
                            "(= {this_wire} (Op2 ({op}) {expr0} {expr1}))\n",
                            this_wire = id_to_wire_name(&id),
                            op = op_node.op.as_str(),
                            expr0 = id_to_wire_name(expr0_id),
                            expr1 = id_to_wire_name(expr1_id)
                            ).as_str()
                        );
                    maybe_push_expr_on_queue(&mut queue, &done, expr0_id);
                    maybe_push_expr_on_queue(&mut queue, &done, expr1_id);
                    },
                    "Extract" => {
                    assert_eq!(term.children.len(), 2);
                    assert_eq!(op_node.children.len(), 2);
                    // TODO: need to think out the semantics of when Extract 
                    // hi != lo
                    // i.e. how to construct the module
                    let hi:i64 = egraph[&op_node.children[0]].op.parse().unwrap();
                    let lo:i64 = egraph[&op_node.children[1]].op.parse().unwrap();
                    assert_eq!(hi, lo);
                    let id = &term.eclass;
                    // TODO: I need to check if an expression is an input here
                    let expr_id = &egraph[&term.children[1]].eclass;

                    let v = inputs.iter().find(|(_k, v)| v == expr_id);
                    let expr = match v {
                     Some((var, _id)) => {
                         let v = input_extract_map.entry(var.clone()).or_default();
                         v.push(hi);

                         format!("{}_{hi}", var)
                     },
                     None => format!("(Op1 (Extract {hi} {lo}) wire_{})", id),
                    };
                    rules.push_str(
                        format!(
                            "(= {this_wire} {expr})\n",
                            this_wire = id_to_wire_name(id),
                            ).as_str()
                        );

                    maybe_push_expr_on_queue(&mut queue, &done, expr_id);

                    },
                    v => todo!("{:?}", v)

                }
            },
            "Var" => { },

            _ => todo!("{:?}", &term),
        }
    }

    // TODO: need to figure out how to do definitions - what does this look like for register.?
    fn vec_list_to_str_cons(v: &Vec<String>) -> String {
        let mut str: String = String::new();
        if v.is_empty() {
            return str;
        }

        for i in v {
            let s = format!("(StringCons \"{i}\" ");
            str.push_str(s.as_str());
        }
        str.push_str("(StringNil)");

        for _i in v {
            str.push(')');
        }

        str
    }
    fn vec_list_to_expr_cons(v: &Vec<String>) -> String {
        let mut str: String = String::new();
        if v.is_empty() {
            return str;
        }

        for i in v {
            let s = format!("(ExprCons {i} ");
            str.push_str(s.as_str());
        }
        str.push_str("(ExprNil)");

        for _i in v {
            str.push(')');
        }

        str
    }

    fn vec_list_to_concat(v: &mut [String]) -> String {
        assert!(v.len() > 0);
        if v.len() == 1 {
            return v[0].clone();
        }
        let mut str: String = String::new();
        // assuming it's [v0, v1, v2...]
        // want (Concat v0 (Concat v1 v0))
        if v.len() == 2 {
            let s = format!("(Op2 (Concat) {} {})", v[0], v[1]);
            str.push_str(s.as_str());
            return s;
        }
        let sz: usize = v.len() - 2;
        for i in &mut v[0..sz] {
            let s = format!("(Op2 (Concat) {i} ");
            str.push_str(s.as_str());
        }
        let s = format!("(Op2 (Concat) {} {})", v[v.len() - 2], v[v.len() - 1]);
        str.push_str(s.as_str());

        for _i in &mut v[0..sz] {
            str.push(')');
        }

        str
    }

    let input_names = inputs.iter().map(|a| a.0.clone()).collect();
    let inputs_str = vec_list_to_str_cons(&input_names);
    let expr_cons = vec_list_to_expr_cons(&input_names);

    let mut maybe_let = String::new();
    let mut vec = input_extract_map.drain().collect::<Vec<_>>();
    vec.sort();

    for (k, v) in &mut vec {
        // let s = format!("(let {} (Wire \"{}\"))", &k, &k);
        // sort the vector
        v.sort();
        let mut v1: Vec<_> = v.iter_mut().map(|bw| format!("{k}_{bw}")).collect();
        let s1 = vec_list_to_concat(&mut v1);
        let s = format!("(let {k} {s1})\n");
        maybe_let.push_str(s.as_str());
    }

    let rule = format!(
        r#"(rule
 ;; set of definitions
 ({rules})
 ;; set of declarations
 (
{maybe_let}
(let instance (ModuleInstance "{name}" (StringNil) (ExprNil) 
                    {inputs_str}
                    {expr_cons}
                    )
      )) :ruleset module_rewrites)"#
    );

    rule
}

pub fn to_verilog(term_dag: &TermDag, id: usize) -> String {
    // let mut wires = HashMap::default();

    fn id_to_wire_name(id: usize) -> String {
        format!("wire_{}", id)
    }

    let mut inputs = String::new();
    let mut logic_declarations = String::new();
    let mut registers = String::new();
    let mut module_declarations = String::new();

    let mut queue = vec![id];
    let mut done = HashSet::new();

    while let Some(id) = queue.pop() {
        done.insert(id);
        let term = term_dag.get(id);

        match term {
            Term::Lit(Literal::String(_)) => (),
            Term::Lit(Literal::Int(v)) => {
                logic_declarations.push_str(&format!(
                    "logic [31:0] {this_wire} = {val};\n",
                    this_wire = id_to_wire_name(id),
                    val = v
                ));
            }
            Term::Var(_) => todo!(),
            Term::App(s, v) => match (s.as_str(), v.as_slice()) {
                ("Reg", &[default_id, clk_id, d_id]) => {
                    let default_val = match term_dag.get(default_id) {
                        Term::Lit(Literal::Int(default_val)) => default_val,
                        _ => panic!(),
                    };

                    logic_declarations.push_str(
                        format!(
                            "logic {this_wire} = {default};\n",
                            this_wire = id_to_wire_name(id),
                            default = default_val
                        )
                        .as_str(),
                    );

                    registers.push_str(&format!(
                        "always @(posedge {clk}) begin
                            {this_wire} <= {d};
                        end\n",
                        clk = id_to_wire_name(clk_id),
                        this_wire = id_to_wire_name(id),
                        d = id_to_wire_name(d_id)
                    ));

                    if !done.contains(&d_id) {
                        queue.push(d_id);
                    }
                    if !done.contains(&clk_id) {
                        queue.push(clk_id);
                    }
                }
                ("Var", [name_id, bw_id]) => {
                    let name = match term_dag.get(*name_id) {
                        Term::Lit(Literal::String(name)) => name,
                        _ => panic!(),
                    };
                    let bw = match term_dag.get(*bw_id) {
                        Term::Lit(Literal::Int(bw)) => bw,
                        _ => panic!(),
                    };

                    inputs.push_str(
                        format!("input [{bw}-1:0] {name};\n", bw = bw, name = name).as_str(),
                    );

                    logic_declarations.push_str(
                        format!(
                            "logic [{bw}-1:0] {this_wire} = {name};\n",
                            bw = bw,
                            this_wire = id_to_wire_name(id),
                            name = name
                        )
                        .as_str(),
                    );
                }
                ("Mux", []) => (),
                ("LUT4", []) => (),
                ("Or", []) => (),
                ("Bitvector", [_]) => (),
                ("Eq", []) => (),
                ("BV", [val_id, bw_id]) => {
                    let val = match term_dag.get(*val_id) {
                        Term::Lit(Literal::Int(val)) => val,
                        _ => panic!(),
                    };
                    let bw = match term_dag.get(*bw_id) {
                        Term::Lit(Literal::Int(bw)) => bw,
                        _ => panic!(),
                    };
                    logic_declarations.push_str(
                        format!(
                            "logic [{bw}-1:0] {this_wire} = {bw}'d{val};\n",
                            bw = bw,
                            this_wire = id_to_wire_name(id),
                            val = val
                        )
                        .as_str(),
                    );
                }
                ("Extract", [hi_id, lo_id, expr_id]) => {
                    let hi = match term_dag.get(*hi_id) {
                        Term::Lit(Literal::Int(hi)) => hi,
                        _ => panic!(),
                    };
                    let lo = match term_dag.get(*lo_id) {
                        Term::Lit(Literal::Int(lo)) => lo,
                        _ => panic!(),
                    };
                    logic_declarations.push_str(&format!(
                        "logic {this_wire} = {expr}[{hi}:{lo}];\n",
                        hi = hi,
                        lo = lo,
                        this_wire = id_to_wire_name(id),
                        expr = id_to_wire_name(*expr_id),
                    ));

                    if !done.contains(expr_id) {
                        queue.push(*expr_id);
                    }
                }
                ("Concat", [expr0_id, expr1_id]) => {
                    logic_declarations.push_str(&format!(
                        "logic {this_wire} = {{ {expr0}, {expr1} }};\n",
                        this_wire = id_to_wire_name(id),
                        expr0 = id_to_wire_name(*expr0_id),
                        expr1 = id_to_wire_name(*expr1_id),
                    ));

                    if !done.contains(expr0_id) {
                        queue.push(*expr0_id);
                    }
                    if !done.contains(expr1_id) {
                        queue.push(*expr1_id);
                    }
                }
                ("ZeroExtend", [expr_id, bw_id]) => {
                    let bw = match term_dag.get(*bw_id) {
                        Term::Lit(Literal::Int(bw)) => bw,
                        _ => panic!(),
                    };
                    logic_declarations.push_str(&format!(
                        "logic {this_wire} = {{ {bw}'d0, {expr} }};\n",
                        this_wire = id_to_wire_name(id),
                        bw = bw,
                        expr = id_to_wire_name(*expr_id),
                    ));

                    if !done.contains(expr_id) {
                        queue.push(*expr_id);
                    }
                }
                ("Sketch1", [op_id, expr_id])
                    if match term_dag.get(*op_id) {
                        Term::App(s, v) => s.as_str() == "LUT4" && v.is_empty(),
                        _ => false,
                    } =>
                {
                    logic_declarations.push_str(&format!(
                        "logic {this_wire};\n",
                        this_wire = id_to_wire_name(id),
                    ));

                    module_declarations.push_str(&format!(
                        "lut4 lut4_{id} (.in({expr}), .out({y}));\n",
                        id = id,
                        expr = id_to_wire_name(*expr_id),
                        y = id_to_wire_name(id),
                    ));

                    if !done.contains(expr_id) {
                        queue.push(*expr_id);
                    }
                }
                _ => todo!("{:?}", (s, v)),
            },
            _ => todo!("{:?}", term),
        }
    }

    format!(
        "module top({inputs});
            {inputs}
            {logic_declarations}
            {registers}
            {module_declarations}
        endmodule",
        inputs = inputs,
        logic_declarations = logic_declarations,
        registers = registers,
        module_declarations = module_declarations,
    )
}

/// Import Churchroad language into an EGraph.
///
/// TODO(@gussmith23): Ideally, this would be done via an `import` statement.
/// That's not currently possible because of the Rust-defined primitive
/// `debruijnify` in Churchroad.
pub fn import_churchroad(egraph: &mut EGraph) {
    // STEP 1: import primary language definitions.
    egraph
        .parse_and_run_program(r#"(include "egglog_src/churchroad.egg")"#)
        .unwrap();

    // STEP 2: add the `debruijnify` primitive to the egraph. This depends on
    // the above language definitions, but it's not possible to do it in egglog,
    // hence it's a Rust function.
    add_debruijnify(egraph);

    // STEP 3: import module enumeration rewrites. These depend on the
    // `debruijnify` primitive.
    egraph
        .parse_and_run_program(r#"(include "egglog_src/module_enumeration_rewrites.egg")"#)
        .unwrap();
}

/// Add the `debruijnify` primitive to an [`EGraph`].
fn add_debruijnify(egraph: &mut EGraph) {
    struct DeBruijnify {
        in_sort: Arc<VecSort>,
        out_sort: Arc<VecSort>,
        i64_sort: Arc<I64Sort>,
    }

    impl PrimitiveLike for DeBruijnify {
        fn name(&self) -> Symbol {
            "debruijnify".into()
        }

        fn get_type_constraints(&self) -> Box<dyn TypeConstraint> {
            Box::new(SimpleTypeConstraint::new(
                self.name(),
                vec![self.in_sort.clone(), self.out_sort.clone()],
            ))
        }

        fn apply(
            &self,
            values: &[crate::Value],
            egraph: Option<&mut EGraph>,
        ) -> Option<crate::Value> {
            let in_vec = Vec::<Value>::load(&self.in_sort, &values[0]);

            let mut seen_values: HashMap<Value, i64> = HashMap::new();
            let mut next_id = 0;
            let mut out = vec![];

            let egraph = egraph.unwrap();

            for value in in_vec {
                // Get representative value.
                let value = egraph.find(value);

                // If we haven't assinged it a number yet, give it the next one.
                seen_values.entry(value).or_insert_with(|| {
                    let id = next_id;
                    next_id += 1;
                    id
                });

                // Add the number to the output vector.
                out.push(seen_values[&value].store(&self.i64_sort).unwrap());
            }

            out.store(&self.out_sort)
        }
    }

    egraph.add_primitive(DeBruijnify {
        i64_sort: egraph.get_sort().unwrap(),
        in_sort: egraph
            .get_sort_by(|s: &Arc<VecSort>| s.name() == "ExprVec".into())
            .unwrap(),
        out_sort: egraph
            .get_sort_by(|s: &Arc<VecSort>| s.name() == "IVec".into())
            .unwrap(),
    });
}

/// Generate all module enumeration rewrites used by Churchroad.
///
/// This function is used to generate the contents of the the
/// `egglog_src/module_enumeration_rewrites.egg` file. A test in this file
/// ensures that the generated file matches what this function produces.
pub fn generate_module_enumeration_rewrites(enumeration_ruleset_name: &str) -> String {
    format!(
            "
(ruleset {enumeration_ruleset_name})
{rewrites}",
            enumeration_ruleset_name = enumeration_ruleset_name,
            rewrites = vec![
                // Var
                // Note that this puts a loop in the graph, because a Var
                // becomes a hole applied to itself. We just need to be careful
                // about that during extraction.
                format!("(rewrite (Var name bw) (apply (MakeModule (Hole) (vec-of 0)) (vec-of (Var_ name bw))) :ruleset {})", enumeration_ruleset_name),

                // 0-ary
                generate_module_enumeration_rewrite(&[], Some(enumeration_ruleset_name)),
                // 1-ary
                generate_module_enumeration_rewrite(&[true], Some(enumeration_ruleset_name)),
                generate_module_enumeration_rewrite(&[false], Some(enumeration_ruleset_name)),
                // 2-ary
                generate_module_enumeration_rewrite(&[true, true], Some(enumeration_ruleset_name)),
                generate_module_enumeration_rewrite(&[true, false], Some(enumeration_ruleset_name)),
                generate_module_enumeration_rewrite(&[false, true], Some(enumeration_ruleset_name)),
                generate_module_enumeration_rewrite(
                    &[false, false],
                    Some(enumeration_ruleset_name)
                ),
                // 3-ary
                generate_module_enumeration_rewrite(
                    &[true, true, true],
                    Some(enumeration_ruleset_name)
                ),
                generate_module_enumeration_rewrite(
                    &[true, true, false],
                    Some(enumeration_ruleset_name)
                ),
                generate_module_enumeration_rewrite(
                    &[true, false, true],
                    Some(enumeration_ruleset_name)
                ),
                generate_module_enumeration_rewrite(
                    &[true, false, false],
                    Some(enumeration_ruleset_name)
                ),
                generate_module_enumeration_rewrite(
                    &[false, true, true],
                    Some(enumeration_ruleset_name)
                ),
                generate_module_enumeration_rewrite(
                    &[false, true, false],
                    Some(enumeration_ruleset_name)
                ),
                generate_module_enumeration_rewrite(
                    &[false, false, true],
                    Some(enumeration_ruleset_name)
                ),
                generate_module_enumeration_rewrite(
                    &[false, false, false],
                    Some(enumeration_ruleset_name)
                ),
                // clang-format on
            ]
            .join("\n"),
        )
}

/// Generate module enumeration rewrite.
///
/// - hole_indicator: a list of booleans indicating whether the Op's
///   argument at the given index is a hole. If true, the argument will
///   become a `(Hole)`. If not, it will expect a module application:
///   `(apply (MakeModule graph indices) args)`.
///
/// ```
/// use churchroad::generate_module_enumeration_rewrite;
/// assert_eq!(generate_module_enumeration_rewrite(&[true, false, true], None),
///           "(rewrite
///   (Op3 op expr0 (apply (MakeModule graph1 _) args1) expr2)
///   (apply (MakeModule (Op3_ op (Hole) graph1 (Hole)) (debruijnify (vec-append (vec-pop (vec-of (Var \"unused\" 0))) (vec-of expr0) args1 (vec-of expr2)))) (vec-append (vec-pop (vec-of (Var \"unused\" 0))) (vec-of expr0) args1 (vec-of expr2)))
/// )");
/// ```
pub fn generate_module_enumeration_rewrite(
    hole_indicator: &[bool],
    ruleset: Option<&str>,
) -> String {
    let arity: usize = hole_indicator.len();

    fn make_apply_pattern(idx: usize) -> String {
        format!("(apply (MakeModule graph{idx} _) args{idx})", idx = idx)
    }

    fn make_opaque_expr_pattern(idx: usize) -> String {
        format!("expr{idx}", idx = idx)
    }

    let arg_patterns = hole_indicator
        .iter()
        .enumerate()
        .map(|(idx, is_hole)| {
            if *is_hole {
                make_opaque_expr_pattern(idx)
            } else {
                make_apply_pattern(idx)
            }
        })
        .collect::<Vec<_>>();

    let lhs = format!(
        "(Op{arity} op {args})",
        arity = arity,
        args = arg_patterns.join(" ")
    );

    let args_rhs_patterns = hole_indicator
        .iter()
        .enumerate()
        .map(|(idx, is_hole)| {
            if *is_hole {
                "(Hole)".to_string()
            } else {
                format!("graph{idx}", idx = idx).to_string()
            }
        })
        .collect::<Vec<_>>();

    // Creates the list of arguments for the module application.
    // the (vec-pop (vec-of ..)) thing is a hack for type inference not working
    let args_list_expr = format!(
        "(vec-append (vec-pop (vec-of (Var \"unused\" 0))) {args})",
        args = hole_indicator
            .iter()
            .enumerate()
            .map(|(idx, is_hole)| {
                if *is_hole {
                    format!("(vec-of expr{idx})", idx = idx)
                } else {
                    format!("args{idx}", idx = idx)
                }
            })
            .collect::<Vec<_>>()
            .join(" ")
    );

    let rhs = format!(
        "(apply (MakeModule (Op{arity}_ op {graphs}) (debruijnify {args})) {args})",
        arity = arity,
        graphs = args_rhs_patterns.join(" "),
        args = args_list_expr,
    );

    format!(
        "(rewrite
  {lhs}
  {rhs}
{ruleset_flag})",
        lhs = lhs,
        rhs = rhs,
        ruleset_flag = match ruleset {
            Some(ruleset) => format!(":ruleset {}\n", ruleset),
            None => "".to_string(),
        },
    )
}

/// List all modules present in the egraph.
pub fn list_modules(egraph: &mut EGraph, num_variants: usize) {
    for s in egraph
        .parse_and_run_program(
            format!("(query-extract :variants {num_variants} (MakeModule mod args))").as_str(),
        )
        .unwrap()
    {
        println!("{}", s);
    }
}

/// Port name, port type, port value.
type Ports = Vec<(String, ArcSort, Value)>;

/// ```
/// use churchroad::*;
/// use egglog::{ArcSort, EGraph, Term, TermDag, Value};
///
/// // Get an egraph, load in a simple design.
/// let mut egraph = EGraph::default();
///
/// import_churchroad(&mut egraph);
/// egraph
///     .parse_and_run_program(
///         r#"
/// ; wire declarations
/// ; $and$<<EOF:2$1_Y
/// (let v0 (Wire "v0" 2))
/// ; a
/// (let v1 (Wire "v1" 2))
/// ; b
/// (let v2 (Wire "v2" 1))
/// ; o
/// (let v3 (Wire "v3" 1))
///
/// ; cells
/// ; TODO not handling signedness
/// (let v4 (Op1 (ZeroExtend 2) v2))
/// (union v0 (Op2 (And) v1 v4))
/// (let v5 (Op1 (Extract 0 0) v0))
/// (union v3 (Op1 (Extract 0 0) v5))
///
/// ; inputs
/// (IsPort "" "a" (Input) (Var "a" 2))
/// (union v1 (Var "a" 2))
/// (IsPort "" "b" (Input) (Var "b" 1))
/// (union v2 (Var "b" 1))
///
/// ; outputs
/// (IsPort "" "o" (Output) v3)
///
/// ; delete wire expressions
/// (delete (Wire "v0" 2))
/// (delete (Wire "v1" 2))
/// (delete (Wire "v2" 1))
/// (delete (Wire "v3" 1))
/// "#,
///     )
///     .unwrap();
///
/// let (inputs, outputs) = get_inputs_and_outputs(&mut egraph);
///
/// // We should have found two inputs, a and b.
/// assert_eq!(inputs.len(), 2);
///
/// fn value_to_string(value: &Value, sort: ArcSort, egraph: &EGraph) -> String {
///     let mut termdag = TermDag::default();
///     let (_, term) = egraph.extract(value.clone(), &mut termdag, &sort);
///     termdag.to_string(&term)
/// }
///
/// // Get expressions for each input.
/// let input_exprs: Vec<String> = inputs
///     .iter()
///     .map(|(_name, sort, value)| value_to_string(value, sort.clone(), &egraph))
///     .collect();
///
/// assert_eq!(input_exprs, vec!["(Var \"a\" 2)", "(Var \"b\" 1)"]);
///
/// let output_expr = value_to_string(&outputs[0].2, outputs[0].1.clone(), &egraph);
/// assert_eq!(output_expr, "(Op1 (Extract 0 0) (Op1 (Extract 0 0) (Op2 (And) (Var \"a\" 2) (Op1 (ZeroExtend 2) (Var \"b\" 1)))))");
/// ```
// TODO(@gussmith23): This really shouldn't require mutability.
pub fn get_inputs_and_outputs(egraph: &mut EGraph) -> (Ports, Ports) {
    // Get the inputs and outputs.
    let mut inputs = vec![];
    let mut outputs = vec![];
    const NUM_TO_GET: usize = 100;
    let (results, termdag) = egraph.function_to_dag("IsPort".into(), NUM_TO_GET).unwrap();
    assert!(results.len() < NUM_TO_GET);
    for (term, output) in &results {
        assert!(
            matches!(output, Term::Lit(Literal::Unit)),
            "IsPort relation shouldn't have any outputs."
        );

        let children = match term {
            Term::App(_, children) => children,
            _ => panic!(),
        };

        let inout_term = children[2];

        enum InOut {
            Input,
            Output,
        }
        let in_or_out = match termdag.get(inout_term) {
            Term::App(in_or_out, v) => {
                assert_eq!(v.len(), 0);
                if in_or_out == "Input".into() {
                    InOut::Input
                } else if in_or_out == "Output".into() {
                    InOut::Output
                } else {
                    panic!()
                }
            }
            _ => panic!(),
        };

        let churchroad_term = children[3];

        let (sort, value) = egraph
            .eval_expr(
                &egglog::ast::parse::ExprParser::new()
                    .parse(&termdag.to_string(&termdag.get(churchroad_term)))
                    .unwrap(),
            )
            .unwrap();

        let port_name = children[1];
        let port_name_str = match termdag.get(port_name) {
            Term::Lit(Literal::String(name)) => name.to_string(),
            _ => panic!(),
        };

        match in_or_out {
            InOut::Input => {
                inputs.push((port_name_str, sort, value));
            }
            InOut::Output => {
                outputs.push((port_name_str, sort, value));
            }
        }
    }

    (inputs, outputs)
}

/// Port name, port eclass.
type PortsFromSerialized = Vec<(String, ClassId)>;

/// ```
/// use churchroad::*;
/// use egglog::{EGraph, SerializeConfig};
///
/// let mut egraph = EGraph::default();
/// import_churchroad(&mut egraph);
/// egraph
///     .parse_and_run_program(
///         r#"
///     ; wire declarations
///     ; $and$<<EOF:2$1_Y
///     (let v0 (Wire "v0" 2))
///     ; a
///     (let v1 (Wire "v1" 2))
///     ; b
///     (let v2 (Wire "v2" 1))
///     ; o
///     (let v3 (Wire "v3" 1))
///
///     ; cells
///     ; TODO not handling signedness
///     (let v4 (Op1 (ZeroExtend 2) v2))
///     (union v0 (Op2 (And) v1 v4))
///     (let v5 (Op1 (Extract 0 0) v0))
///     (union v3 (Op1 (Extract 0 0) v5))
///
///     ; inputs
///     (IsPort "" "a" (Input) (Var "a" 2))
///     (union v1 (Var "a" 2))
///     (IsPort "" "b" (Input) (Var "b" 1))
///     (union v2 (Var "b" 1))
///
///     ; outputs
///     (IsPort "" "o" (Output) v3)
///
///     ; delete wire expressions
///     (delete (Wire "v0" 2))
///     (delete (Wire "v1" 2))
///     (delete (Wire "v2" 1))
///     (delete (Wire "v3" 1))
///     "#,
///     )
///     .unwrap();
///
/// let serialized = egraph.serialize(SerializeConfig::default());
/// let (inputs, outputs) = get_inputs_and_outputs_serialized(&serialized);
///
/// // We should have found two inputs, a and b.
/// assert_eq!(inputs.len(), 2);
/// assert_eq!(inputs[0].0, "a");
/// assert_eq!(inputs[1].0, "b");
///
/// // We should have found one output, o.
/// assert_eq!(outputs.len(), 1);
/// assert_eq!(outputs[0].0, "o");
/// ```
pub fn get_inputs_and_outputs_serialized(
    egraph: &egraph_serialize::EGraph,
) -> (PortsFromSerialized, PortsFromSerialized) {
    // Find IsPort relations.
    #[derive(Clone)]
    enum InputOrOutput {
        Input(String, ClassId),
        Output(String, ClassId),
    }

    fn is_port(node: &Node, egraph: &egraph_serialize::EGraph) -> Option<InputOrOutput> {
        if node.op != "IsPort" {
            return None;
        }

        assert_eq!(node.children.len(), 4);

        let inout = &node.children[2];

        let expr = egraph[&node.children[3]].eclass.clone();

        let name = egraph[&node.children[1]]
            .op
            .strip_prefix('\"')
            .unwrap()
            .strip_suffix('\"')
            .unwrap()
            .to_string();

        match egraph[inout].op.as_str() {
            "Input" => Some(InputOrOutput::Input(name, expr)),
            "Output" => Some(InputOrOutput::Output(name, expr)),
            _ => panic!(),
        }
    }

    let inputs_and_outputs = egraph
        .nodes
        .iter()
        .filter_map(|(_id, node)| is_port(node, egraph))
        .collect::<Vec<_>>();

    let inputs = inputs_and_outputs
        .iter()
        .filter_map(|io| match io {
            InputOrOutput::Input(n, v) => Some((n.clone(), v.clone())),
            _ => None,
        })
        .collect::<Vec<_>>();
    let outputs = inputs_and_outputs
        .iter()
        .filter_map(|io| match io {
            InputOrOutput::Output(n, v) => Some((n.clone(), v.clone())),
            _ => None,
        })
        .collect::<Vec<_>>();

    (inputs, outputs)
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::path::Path;

    use egglog::{EGraph, SerializeConfig};

    /// Doing some exploration of where cyclic extraction breaks in egglog with
    /// Andrew and Vishal.
    #[test]
    fn generate_loop() {
        let mut egraph = EGraph::default();
        import_churchroad(&mut egraph);

        egraph
            .parse_and_run_program(
                r#"
                (let placeholder (Wire "placeholder" 8))
                (let reg (Op1 (Reg 0) placeholder))
                (union placeholder reg)
                (delete (Wire "placeholder" 8))
            "#,
            )
            .unwrap();

        // Uncomment to write out the SVG.
        // let serialized = egraph.serialize_for_graphviz(true);
        // let svg_path = Path::new("tmp").with_extension("svg");
        // serialized.to_svg_file(svg_path).unwrap();

        // Extract reg from Egraph.
        let mut _termdag = TermDag::default();
        let (_sort, _value) = egraph
            .eval_expr(&egglog::ast::Expr::Var((), "reg".into()))
            .unwrap();
        // This will panic, which is what we were trying to get to.
        // It panics with `No cost for Value { tag: "Expr", bits: 6 }`
        // which is basically egglog saying that it can't get a cost because
        // of the cycle. I expected it to loop infinitely, but it's smarter than
        // that.
        // let (_, extracted) = egraph.extract(_value, &mut _termdag, &_sort);

        // Next: can we serialize the egraph? That's the first step to building
        // a new extraction algorithm.
    }

    #[test]
    fn test_module_enumeration_rewrites_up_to_date() {
        // Read in egglog_src/module_enumeration_rewrites.egg and check that it
        // matches the output of generate_module_enumeration_rewrites.
        let actual = std::fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("egglog_src")
                .join("module_enumeration_rewrites.egg"),
        )
        .unwrap();
        let expected = super::generate_module_enumeration_rewrites("enumerate-modules");
        assert_eq!(
            expected, actual,
            "Copy and paste this up-to-date source into module_enumeartion_rewrites.egg:\n{}",
            expected
        );
    }

    #[test]
    fn demo_2024_02_06() {
        // Set the environment variable DEMO_2024_02_06_WRITE_SVGS to anything
        // to produce SVGs.
        fn write_svg(egraph: &EGraph, path: &str) {
            if std::env::var("DEMO_2024_02_06_WRITE_SVGS").is_err() {
                return;
            }
            let serialized = egraph.serialize_for_graphviz(true);
            let svg_path = Path::new(path).with_extension("svg");
            serialized.to_svg_file(svg_path).unwrap();
        }

        ///////////////////////////// BEGIN DEMO ///////////////////////////////

        // We currently need to import Churchroad via Rust (rather than using an
        // egglog `include`) because it depends on a custom primitive.
        let mut egraph = EGraph::default();
        import_churchroad(&mut egraph);

        // Churchroad programs can be very simple circuits, e.g. this one-bit and:
        egraph
            .parse_and_run_program(
                r#"

                (let one-bit-and (Op2 (And) (Var "a" 1) (Var "b" 1)))

            "#,
            )
            .unwrap();
        write_svg(&egraph, "1.svg");

        // Clean up the last example...
        let mut egraph = EGraph::default();
        import_churchroad(&mut egraph);

        // The first interesting feature of Churchroad is that it can represent
        // cyclic circuits using the native features of the egraph. For example,
        // a simple counter circuit looks like this:
        //
        //        ┌────┐
        //      ┌─▼─┐ ┌┴─┐
        //      │reg│ │+1│
        //      └─┬─┘ └▲─┘
        //        └────┘
        //
        // In Churchroad, we can capture this easily using the following
        // commands:
        egraph
            .parse_and_run_program(
                r#"

                ; Instantiate a placeholder wire, which will be connected later.
                (let placeholder (Wire "placeholder" 8))

                ; Generate the +1 box, but feed it with a temporary placeholder.
                (let plusone  (Op2 (Add) placeholder (Op0 (BV 1 8))))

                ; Generate the register, whose input is the output of +1.
                (let reg (Op1 (Reg 0) plusone))

                ; Finally, connect the placeholder to the output of the register
                ; and delete the placeholder.
                (union placeholder reg)
                (delete (Wire "placeholder" 8))

            "#,
            )
            .unwrap();
        write_svg(&egraph, "2.svg");

        // Clean up the last example...
        let mut egraph = EGraph::default();
        import_churchroad(&mut egraph);

        // The next interesting feature of Churchroad is that the representation
        // and its rewrites allow it to find repeated patterns across the
        // egraph.
        //
        // First, let's discuss the underlying representation that allows this.
        // As we saw in the first example, Churchroad can represent circuits
        // directly. However, Churchroad can also represent circuits as
        // applications of abstract modules to concrete inputs:
        egraph
            .parse_and_run_program(
                r#"

                ; An abstract `and` module.
                (let and-module (MakeModule (Op2_ (And) (Hole) (Hole)) (vec-of 0 1)))

                ; We can represent a concrete `and` by applying the abstract
                ; module to concrete inputs.
                (let and (apply and-module (vec-of (Var "a" 1) (Var "b" 1))))

            "#,
            )
            .unwrap();
        write_svg(&egraph, "3.svg");

        // Clean up the last example...
        let mut egraph = EGraph::default();
        import_churchroad(&mut egraph);

        // Translating from the first form to the second (`apply`-based) form is
        // achieved simply with rewrites!
        egraph
            .parse_and_run_program(
                r#"

                ; First, "direct" form.
                (let and (Op2 (And) (Var "a" 1) (Var "b" 1)))

                ; Run module enumeration rewrites to convert to "apply" form.
                (run-schedule (repeat 1 enumerate-modules))
    
            "#,
            )
            .unwrap();
        write_svg(&egraph, "4.svg");

        // Clean up the last example...
        let mut egraph = EGraph::default();
        import_churchroad(&mut egraph);

        // So why do this? Well the `apply`-based form allows us to find
        // repeated patterns in the egraph. As a simple example, imagine we have
        // a series of two `and` gates in a row. This form will allow us to
        // discover that the two `and` gates are the same:
        egraph
            .parse_and_run_program(
                r#"

                ; First, "direct" form.
                (let and (Op2 (And) (Var "a" 1) (Op2 (And) (Var "b" 1) (Var "c" 1))))

                ; Run module enumeration rewrites to convert to "apply" form.
                (run-schedule (saturate enumerate-modules))
    
            "#,
            )
            .unwrap();
        write_svg(&egraph, "5.svg");
    }

    #[test]
    fn test_module_instance() {
        let mut egraph = EGraph::default();
        import_churchroad(&mut egraph);
        egraph.parse_and_run_program(r#"
            ; wire declarations
            ; a
            (let v0 (Wire "v0" 1))
            ; b
            (let v1 (Wire "v1" 1))
            ; out
            (let v2 (Wire "v2" 1))

            ; cells
            (let some_module_instance (ModuleInstance "some_module" (StringCons "p" (StringNil)) (ExprCons (Op0 (BV 4 4)) (ExprNil)) (StringCons "a" (StringCons "b" (StringNil))) (ExprCons v0 (ExprCons v1 (ExprNil)))))
            (union (GetOutput some_module_instance "out") v2)

            ; inputs
            (IsPort "" "a" (Input) (Var "a" 1))
            (union v0 (Var "a" 1))
            (IsPort "" "b" (Input) (Var "b" 1))
            (union v1 (Var "b" 1))

            ; outputs
            (IsPort "" "out" (Output) v2)

            ; delete wire expressions
            (delete (Wire "v0" 1))
            (delete (Wire "v1" 1))
            (delete (Wire "v2" 1))
            "#).unwrap();
    }

    #[test]
    fn extract_cycle() {
        let mut egraph = EGraph::default();
        import_churchroad(&mut egraph);

        egraph
            .parse_and_run_program(
                r#"
                (let placeholder (Wire "placeholder" 8))
                (let reg (Op1 (Reg 0) placeholder))
                (union placeholder reg)
                (delete (Wire "placeholder" 8))
                (IsPort "" "out" (Output) reg)
            "#,
            )
            .unwrap();

        let serialized = egraph.serialize(SerializeConfig::default());
        let out = AnythingExtractor.extract(&serialized, &[]);

        // TODO(@gussmith23) terrible assertion, but it's a start.
        assert_eq!(
            "module top(
  
  
  output out,
);
  logic out = wire_6;
  logic wire_6 = 0;
  
always @(posedge clk) begin
                            wire_6 <= wire_6;
                        end


endmodule",
            to_verilog_egraph_serialize(&serialized, &out, "clk")
        );
    }

    #[test]
    fn compile_module_instance() {
        let mut egraph = EGraph::default();
        import_churchroad(&mut egraph);

        egraph
            .parse_and_run_program(
                r#"
                (let a (Var "a" 8))
                (IsPort "" "a" (Input) a)
                (let b (Var "b" 8))
                (IsPort "" "b" (Input) b)
                (IsPort "" "out" (Output) (GetOutput (ModuleInstance "some_module" (StringCons "p" (StringNil)) (ExprCons (Op0 (BV 4 4)) (ExprNil)) (StringCons "a" (StringCons "b" (StringNil))) (ExprCons a (ExprCons b (ExprNil)))) "out"))
            "#,
            )
            .unwrap();

        let serialized = egraph.serialize(SerializeConfig::default());
        let out = AnythingExtractor.extract(&serialized, &[]);

        assert_eq!(
            "module top(
  
  input [8-1:0] a,
  input [8-1:0] b,
  
  output out,
);
  logic out = wire_23;
  logic wire_23;
  logic [4-1:0] wire_15 = 4'd4;
  logic [8-1:0] wire_9 = b;
  logic [8-1:0] wire_6 = a;
  

  some_module #(
    .p(wire_15)
) module_22 (
    .a(wire_6),
    .b(wire_9),
    .out(wire_23));
endmodule",
            to_verilog_egraph_serialize(&serialized, &out, "")
        );
    }

    #[test]
    fn get_inputs_and_outputs_with_cycle() {
        let mut egraph = EGraph::default();
        import_churchroad(&mut egraph);

        egraph
            .parse_and_run_program(
                r#"
                (let placeholder (Wire "placeholder" 8))
                (let reg (Op1 (Reg 0) placeholder))
                (union placeholder reg)
                (delete (Wire "placeholder" 8))
                (IsPort "" "out" (Output) reg)
            "#,
            )
            .unwrap();

        get_inputs_and_outputs_serialized(&egraph.serialize(SerializeConfig::default()));
    }
    #[test]
    fn compile_rewrite_rule() {
        let mut egraph = EGraph::default();
        import_churchroad(&mut egraph);
        egraph
            .parse_and_run_program(
                r#"
(let v0 (Wire "v0" 4))
; clk
(let v1 (Wire "v1" 1))
; out
(let v2 (Wire "v2" 4))

; cells
; 1'1
(let v3 (Op0 (BV 1 1)))
; TODO not handling signedness
(let v4 (Op1 (ZeroExtend 4) v3))
(union v0 (Op2 (Add) v2 v4))
; TODO: assuming 0 default for Reg
(union v2 (Op2 (Reg 0) v1 v0))

; inputs
(let clk (Var "clk" 1))


(IsPort "" "clk" (Input) clk)
(union v1 clk)

; outputs
(let out v2)
(IsPort "" "out" (Output) out)

; delete wire expressions
(delete (Wire "v0" 4))
(delete (Wire "v1" 1))
(delete (Wire "v2" 4))
                "#,
            )
            .unwrap();

        let serialized = egraph.serialize(SerializeConfig::default());
        let imap = AnythingExtractor.extract(&serialized, &[]);

        let out = to_rewrite_rule_egraph_serialize(&serialized, &imap, "REG");
        println!("{out}");

        assert_eq!(
            r#"(rule
 ;; set of definitions
 ((= wire_24 (Op2 (Reg 0) clk wire_6))
(= wire_6 (Op2 (Add) wire_24 wire_15))
(= wire_15 (Op1 (ZeroExtend 4) wire_12))
(= wire_12 (Op0 (BV 1 1)))
)
 ;; set of declarations
 (

(let instance (ModuleInstance "REG" (StringNil) (ExprNil) 
                    (StringCons "clk" (StringNil))
                    (ExprCons clk (ExprNil))
                    )
      )) :ruleset module_rewrites)"#,
            out
        );
    }

    #[test]
    fn compile_rewrite_rule_1() {
        let mut egraph = EGraph::default();
        import_churchroad(&mut egraph);
        egraph
            .parse_and_run_program(
                r#"
; wire declarations
; $abc$84$auto$blifparse.cc:396:parse_blif$85.A
(let v0 (Wire "v0" 1))
; $abc$84$auto$blifparse.cc:396:parse_blif$85.Y
(let v1 (Wire "v1" 1))
; $abc$84$auto$blifparse.cc:396:parse_blif$86.A
(let v2 (Wire "v2" 1))
; $abc$84$auto$blifparse.cc:396:parse_blif$86.Y
(let v3 (Wire "v3" 1))
; $abc$84$auto$blifparse.cc:396:parse_blif$87.B
(let v4 (Wire "v4" 1))
; $abc$84$auto$blifparse.cc:396:parse_blif$87.Y
(let v5 (Wire "v5" 1))
; $abc$84$auto$blifparse.cc:396:parse_blif$88.B
(let v6 (Wire "v6" 1))
; $abc$84$auto$blifparse.cc:396:parse_blif$88.Y
(let v7 (Wire "v7" 1))
; i_a
(let v8 (Wire "v8" 2))
; i_b
(let v9 (Wire "v9" 2))
; o_res
(let v10 (Wire "v10" 2))

; cells
(union v10 (Op2 (Concat) v7 v5))
(union v4 (Op1 (Extract 0 0) v9))
(union v6 (Op1 (Extract 1 1) v9))
(union v0 (Op1 (Extract 0 0) v8))
(union v2 (Op1 (Extract 1 1) v8))
(union v1 (Op1 (Not) v0))
(union v3 (Op1 (Not) v2))
(union v5 (Op2 (Xor) v1 v4))
(union v7 (Op2 (Xor) v3 v6))

; inputs
(let i_a (Var "i_a" 2))
(IsPort "" "i_a" (Input) i_a)
(union v8 i_a)
(let i_b (Var "i_b" 2))
(IsPort "" "i_b" (Input) i_b)
(union v9 i_b)

; outputs
(let o_res v10)
(IsPort "" "o_res" (Output) o_res)

; delete wire expressions
(delete (Wire "v0" 1))
(delete (Wire "v1" 1))
(delete (Wire "v2" 1))
(delete (Wire "v3" 1))
(delete (Wire "v4" 1))
(delete (Wire "v5" 1))
(delete (Wire "v6" 1))
(delete (Wire "v7" 1))
(delete (Wire "v8" 2))
(delete (Wire "v9" 2))
(delete (Wire "v10" 2))
                "#,
            )
            .unwrap();

        let serialized = egraph.serialize(SerializeConfig::default());
        let imap = AnythingExtractor.extract(&serialized, &[]);

        let out = to_rewrite_rule_egraph_serialize(&serialized, &imap, "ALU");
        println!("\n{out}\nend");
        assert_eq!(
            r#"(rule
 ;; set of definitions
 ((= wire_46 (Op2 (Concat) wire_20 wire_16))
(= wire_16 (Op2 (Xor) wire_8 wire_14))
(= wire_14 i_b_0)
(= wire_8 (Op1 (Not)  wire_6))
(= wire_6 i_a_0)
(= wire_20 (Op2 (Xor) wire_12 wire_18))
(= wire_18 i_b_1)
(= wire_12 (Op1 (Not)  wire_10))
(= wire_10 i_a_1)
)
 ;; set of declarations
 (
(let i_a (Op2 (Concat) i_a_0 i_a_1))
(let i_b (Op2 (Concat) i_b_0 i_b_1))

(let instance (ModuleInstance "ALU" (StringNil) (ExprNil) 
                    (StringCons "i_a" (StringCons "i_b" (StringNil)))
                    (ExprCons i_a (ExprCons i_b (ExprNil)))
                    )
      )) :ruleset module_rewrites)"#,
            out
        )
        // println!("rule:\n {out}");
    }
}
