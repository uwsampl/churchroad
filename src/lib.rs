use egraph_serialize::{ClassId, Node, NodeId};
use indexmap::IndexMap;
use log::info;
use std::{
    collections::{HashMap, HashSet},
    env,
    fmt::Debug,
    fs::read_to_string,
    io::Write,
    path::Path,
    process::{Command, Stdio},
    sync::Arc,
    time::SystemTime,
};
use tempfile::NamedTempFile;

use egglog::{
    ast::{Literal, Symbol},
    constraint::{SimpleTypeConstraint, TypeConstraint},
    sort::{FromSort, I64Sort, IntoSort, Sort, VecSort},
    ArcSort, EGraph, PrimitiveLike, Term, TermDag, Value,
};

pub fn call_lakeroad_on_primitive_interface_and_spec(
    serialized_egraph: &egraph_serialize::EGraph,
    spec_choices: &indexmap::IndexMap<egraph_serialize::ClassId, egraph_serialize::NodeId>,
    _spec_node_id: &NodeId,
    sketch_template_node_id: &NodeId,
    architecture: &str,
) -> String {
    // Not supporting anything other than a simple DSP sketch at the moment.
    assert_eq!(
        serialized_egraph[sketch_template_node_id].op,
        "PrimitiveInterfaceDSP"
    );
    assert_eq!(serialized_egraph[sketch_template_node_id].children.len(), 2);
    assert_eq!(
        {
            let child_node_id = &serialized_egraph[sketch_template_node_id].children[0];
            &serialized_egraph[child_node_id].op
        },
        "Var"
    );
    assert_eq!(
        {
            let child_node_id = &serialized_egraph[sketch_template_node_id].children[1];
            &serialized_egraph[child_node_id].op
        },
        "Var"
    );

    // Put the spec in a tempfile.
    let mut spec_file = NamedTempFile::new().unwrap();
    let spec_filepath = spec_file.path().to_owned();
    spec_file
        .write_all(to_verilog_egraph_serialize(serialized_egraph, spec_choices, "clk").as_bytes())
        .unwrap();
    spec_file.flush().unwrap();
    // spec_file.persist("tmp.v").unwrap();

    let binding =
        env::var("LAKEROAD_DIR").expect("LAKEROAD_DIR environment variable should be set.");
    let lakeroad_dir = Path::new(&binding);
    let mut command = Command::new("racket");
    command
        .arg(lakeroad_dir.join("bin").join("main.rkt"))
        .arg("--architecture")
        .arg(architecture)
        .arg("--verilog-module-filepath")
        .arg(spec_filepath)
        // TODO(@gussmith23): This should probably not be implicit in the generate-verilog function.
        .arg("--top-module-name")
        .arg("top")
        // TODO(@gussmith23): Determine this automatically somehow.
        .arg("--verilog-module-out-signal")
        .arg("out:16")
        .arg("--input-signal")
        .arg("a:16")
        .arg("--input-signal")
        .arg("b:16")
        .arg("--template")
        .arg("dsp")
        .arg("--pipeline-depth")
        .arg("0")
        .arg("--out-format")
        .arg("verilog")
        .arg("--timeout")
        .arg("120");
    // dbg!(&command
    //     .get_args()
    //     .map(|s| s.to_str().unwrap().to_owned())
    //     .collect::<Vec<_>>()
    //     .join(" "));
    let output = command.output().unwrap();

    if !output.status.success() {
        panic!(
            "Lakeroad failed with code {}. stdout:\n{}\n\nstderr:\n{}",
            output.status.code().unwrap(),
            String::from_utf8_lossy(&output.stderr),
            String::from_utf8_lossy(&output.stdout)
        );
    }

    let mut verilog = String::from_utf8_lossy(&output.stdout).into_owned();

    info!(
        "First few lines of generated Verilog:\n{}",
        verilog.lines().take(10).collect::<Vec<_>>().join("\n")
    );

    // TODO(@gussmith23): hardcoding the definition of DSP48E2 here. Should
    // instead take it as input to the flow at some point.
    verilog.push_str(
        r#"
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
"#,
    );

    let choices = AnythingExtractor.extract(serialized_egraph, &[]);

    // Generate port bindings.
    let mut port_to_expr_map = HashMap::new();
    port_to_expr_map.insert(
        "a".to_string(),
        node_to_string(
            serialized_egraph,
            &serialized_egraph[sketch_template_node_id].children[0],
            &choices,
        ),
    );
    port_to_expr_map.insert(
        "b".to_string(),
        node_to_string(
            serialized_egraph,
            &serialized_egraph[sketch_template_node_id].children[1],
            &choices,
        ),
    );
    port_to_expr_map.insert(
        "out".to_string(),
        node_to_string(serialized_egraph, sketch_template_node_id, &choices),
    );

    // TODO(@gussmith23): hardcoded module name
    // Don't simcheck, because there'll be undefined modules. That's expected.
    // Don't generate let bindings.

    commands_from_verilog(&verilog, "top", false, false, port_to_expr_map)
}

/// Converts serialized egraph with map of choices and root node to an S-expr.
/// TODO(@gussmith23): this will probably run infinitely when there are cycles.
pub fn node_to_string(
    egraph: &egraph_serialize::EGraph,
    node_id: &NodeId,
    choices: &indexmap::IndexMap<egraph_serialize::ClassId, egraph_serialize::NodeId>,
) -> String {
    let node = &egraph.nodes[node_id];
    let mut out = String::new();
    // Don't wrap in parens when op begins with a quote or a number.
    // TODO(@gussmith23): this is very rough.
    let wrap_in_parens = node.op.chars().next().unwrap().is_ascii_alphabetic();

    if wrap_in_parens {
        out += "(";
    }
    out += node.op.as_str();
    for child_id in &node.children {
        out += " ";
        out += &node_to_string(egraph, &choices[&egraph[child_id].eclass], choices);
    }

    if wrap_in_parens {
        out += ")";
    }

    out
}

// TODO(@gussmith23): It would be nice to not have to use a mutable reference
// here.
pub fn find_primitive_interface_values(egraph: &mut EGraph) -> Vec<(ArcSort, Value)> {
    const NUM_TO_GET: usize = 100;
    let (results, termdag) = egraph
        .function_to_dag("PrimitiveInterfaceDSP".into(), NUM_TO_GET)
        .unwrap();
    assert!(results.len() < NUM_TO_GET);

    for (term, output) in &results {
        // I don't know if this is actually always the case, but I don't know
        // what it means when it's not the case.
        assert_eq!(term, output);
    }

    let sorts_and_exprs: Vec<_> = results
        .iter()
        .map(|(term, output)| {
            // I don't know if this is actually always the case, but I don't know
            // what it means when it's not the case.
            assert_eq!(term, output);

            egraph.eval_expr(&termdag.term_to_expr(term)).unwrap()
        })
        .collect();

    sorts_and_exprs
}

/// Same as [`find_primitive_interface_values`] but over a serialized egraph.
pub fn find_primitive_interfaces_serialized(egraph: &egraph_serialize::EGraph) -> Vec<NodeId> {
    egraph
        .nodes
        .iter()
        .filter_map(|(node_id, node)| {
            if node.op == "PrimitiveInterfaceDSP" {
                Some(node_id.to_owned())
            } else {
                None
            }
        })
        .collect()
}

/// Extracts only expressions legal in structural Verilog.
#[derive(Default)]
pub struct StructuralVerilogExtractor;
impl StructuralVerilogExtractor {
    pub fn extract(
        &self,
        egraph: &egraph_serialize::EGraph,
        _roots: &[egraph_serialize::ClassId],
    ) -> IndexMap<egraph_serialize::ClassId, egraph_serialize::NodeId> {
        egraph
            .classes()
            .iter()
            .map(|(id, class)| {
                let mut potential_nodes = class
                    .nodes
                    .iter()
                    .filter(|node_id| {
                        let op = &egraph[*node_id].op;
                        // Filter certain op types.
                        !(["PrimitiveInterfaceDSP"].contains(&op.as_str()))
                    })
                    .filter(|node_id| {
                        let op = egraph[*node_id].op.as_str();
                        match op {
                            "Op0" | "Op1" | "Op2" | "Op3" => {
                                let op_name = &egraph[*node_id].children[0];
                                matches!(
                                    egraph[op_name].op.as_str(),
                                    "Extract"
                                        | "Concat"
                                        | "BV"
                                        | "CRString"
                                        | "ZeroExtend"
                                        | "SignExtend"
                                )
                            }
                            _ => true,
                        }
                    })
                    .collect::<Vec<_>>();
                potential_nodes.sort_by(|a, b| {
                    // If either is a wire, it should come last.
                    let are_wires = (egraph[*a].op == "Wire", egraph[*b].op == "Wire");
                    match are_wires {
                        (true, true) => std::cmp::Ordering::Equal,
                        (true, false) => std::cmp::Ordering::Greater,
                        (false, true) => std::cmp::Ordering::Less,
                        (false, false) => {
                            // Otherwise, sort by the node ID.
                            a.cmp(b)
                        }
                    }
                });
                assert!(!potential_nodes.is_empty(), "Found unextractable class.");
                let node_id = potential_nodes[0].clone();
                // Warn if we're still extracting a Wire.
                if egraph[&node_id].op == "Wire" {
                    log::warn!("Extracting a Wire.");
                }
                (id.clone(), node_id)
            })
            .collect()
    }
}

/// I don't know if we should be making Extractors in such an ad-hoc way, but
/// this actually seems to be the most convenient way to do this.
///
/// An extractor for extracting things we can use as specifications. This mostly
/// means that it extract things that we can turn into legal Verilog.
#[derive(Default)]
pub struct SpecExtractor;
impl SpecExtractor {
    pub fn extract(
        &self,
        egraph: &egraph_serialize::EGraph,
        _roots: &[egraph_serialize::ClassId],
    ) -> IndexMap<egraph_serialize::ClassId, egraph_serialize::NodeId> {
        egraph
            .classes()
            .iter()
            .map(|(id, class)| {
                let node_id = class
                    .nodes
                    .iter()
                    .find(|node_id| {
                        let op = &egraph[*node_id].op;

                        // Filter certain op types.
                        !(["PrimitiveInterfaceDSP", "Wire"].contains(&op.as_str()))
                    })
                    .unwrap()
                    .clone();
                (id.clone(), node_id)
            })
            .collect()
    }
}

/// For a given primitive interface term `term`, find an equivalent "concrete"
/// expression that we can use as our spec for synthesis via Lakeroad.
///
/// Returns the node choides for each node in the egraph, plus the specific node
/// ID representing the spec.
pub fn find_spec_for_primitive_interface(
    eclass: &egraph_serialize::ClassId,
    serialized_egraph: &egraph_serialize::EGraph,
) -> (IndexMap<ClassId, NodeId>, NodeId) {
    let choices = SpecExtractor.extract(serialized_egraph, vec![].as_slice());
    let node_id = choices.get(eclass).unwrap().to_owned();
    (choices, node_id)
}

pub fn call_lakeroad_on_primitive_interface(term: &Term, term_dag: &TermDag) {
    dbg!(term_dag.term_to_expr(term).to_string());

    dbg!(to_verilog(term_dag, term_dag.lookup(term)));
}

pub fn call_lakeroad() {}

/// ```
/// let mut egraph = churchroad::from_verilog("module identity(input i, output o); assign o = i; endmodule", "identity");
/// egraph.parse_and_run_program(r#"(check (IsPort "" "i" (Input) i) (IsPort "" "o" (Output) o) (= i o))"#);
/// ```
pub fn from_verilog(
    verilog: &str,
    top_module_name: &str,
    simcheck: bool,
    let_bindings: bool,
    port_to_expr_map: HashMap<String, String>,
) -> EGraph {
    let mut f = NamedTempFile::new().unwrap();
    f.write_all(verilog.as_bytes()).unwrap();
    from_verilog_file(
        f.path(),
        top_module_name,
        simcheck,
        let_bindings,
        port_to_expr_map,
    )
}

/// Version of [`from_verilog`] that takes a filepath argument.
pub fn from_verilog_file(
    verilog_filepath: &Path,
    top_module_name: &str,
    simcheck: bool,
    let_bindings: bool,
    port_to_expr_map: HashMap<String, String>,
) -> EGraph {
    let mut egraph = EGraph::default();
    import_churchroad(&mut egraph);
    egraph
        .parse_and_run_program(&commands_from_verilog_file(
            verilog_filepath,
            top_module_name,
            simcheck,
            let_bindings,
            port_to_expr_map,
        ))
        .unwrap();

    egraph
}

pub fn commands_from_verilog(
    verilog: &str,
    top_module_name: &str,
    simcheck: bool,
    let_bindings: bool,
    port_to_expr_map: HashMap<String, String>,
) -> String {
    let mut f = NamedTempFile::new().unwrap();
    f.write_all(verilog.as_bytes()).unwrap();
    commands_from_verilog_file(
        f.path(),
        top_module_name,
        simcheck,
        let_bindings,
        port_to_expr_map,
    )
}

/// simcheck: whether to run `hierarchy` with `-simcheck`.
pub fn commands_from_verilog_file(
    verilog_filepath: &Path,
    top_module_name: &str,
    simcheck: bool,
    let_bindings: bool,
    port_to_expr_map: HashMap<String, String>,
) -> String {
    fn spaces_to_underscores(s: &str) -> String {
        assert!(!s.contains('_'));
        s.replace(' ', "_")
    }
    // Convert the port_to_expr_map into "-portunion key value".
    let port_union_flags: String = port_to_expr_map
        .iter()
        // TODO(@gussmith23): qouting here is a problem...
        .map(|(port_name, expr)| {
            format!("-portunion {} {}", port_name, spaces_to_underscores(expr))
        })
        .collect::<Vec<_>>()
        .join(" ");

    let logfile = NamedTempFile::new().unwrap();
    // TODO(@gussmith23): hardcoded .so will break on other systems.
    let command_output = Command::new("yosys")
        .arg("-m")
        .arg(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("yosys-plugin")
                .join("churchroad.so"),
        )
        // Quiet, so the only output is Churchroad egglog code.
        .arg("-q")
        // Log useful information to a file.
        .arg("-l")
        .arg(logfile.path())
        .arg("-p")
        .arg(format!(
            "read_verilog {path}; hierarchy {simcheck} -top {top_module_name}; write_churchroad -salt {salt} {let_bindings_flag} {port_union_flags}",
            path = verilog_filepath.to_str().unwrap(),
            simcheck = if simcheck { "-simcheck" } else { "" },
            salt = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_nanos(),
            let_bindings_flag = if let_bindings { "-letbindings" } else { "" },
            port_union_flags = port_union_flags,
        ))
        .stdout(Stdio::piped())
        .output()
        .expect("Couldn't run Yosys.");

    if !command_output.status.success() {
        panic!(
            "Yosys failed. Log:\n{}",
            read_to_string(logfile.path()).unwrap()
        );
    }

    String::from_utf8_lossy(&command_output.stdout).into_owned()
}

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
        Some((id, _)) => interpret_helper(egraph, id, time, env, &mut HashMap::default()),
        None => return Err("No class with the given ID.".to_string()),
    };

    result
}

pub fn get_bitwidth_for_node(
    egraph: &egraph_serialize::EGraph,
    id: &NodeId,
) -> Result<u64, String> {
    match egraph.nodes.iter().find(|(_, node)| {
        node.op.as_str() == "HasType" && egraph[&node.children[0]].eclass == egraph[id].eclass
    }) {
        Some((_, has_type_node)) => {
            let type_node = egraph.nodes.get(&has_type_node.children[1]).unwrap();
            assert!(type_node.op == "Bitvector");

            let bw: u64 = egraph
                .nodes
                .get(&type_node.children[0])
                .unwrap()
                .op
                .parse()
                .unwrap();
            Ok(bw)
        }
        None => Err("No HasType node found for the given ID.".to_string()),
    }
}

fn truncate_value_to_bitwidth(val: u64, bw: u64) -> u64 {
    assert!(bw <= 64);
    assert!(bw > 0);
    if bw == 64 {
        val
    } else {
        val & ((1 << bw) - 1)
    }
}

fn interpret_helper(
    egraph: &egraph_serialize::EGraph,
    id: &ClassId,
    time: usize,
    env: &HashMap<&str, Vec<u64>>,
    cache: &mut HashMap<(ClassId, usize), InterpreterResult>,
) -> Result<InterpreterResult, String> {
    if cache.contains_key(&(id.clone(), time)) {
        return Ok(cache[&(id.clone(), time)].clone());
    }
    let node_ids = &egraph.classes().get(id).unwrap().nodes;
    if node_ids.len() != 1 {
        return Err(format!(
            "There should be exactly one node in the class, but there are {}.",
            node_ids.len()
        ));
    }

    let node_id = node_ids.first().unwrap();
    let node = egraph.nodes.get(node_id).unwrap();

    let result = match node.op.as_str() {
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
                    .unwrap_or_else(|| panic!("no value at time {:?}", time)),
                bw,
            ))
        }
        "Op0" | "Op1" | "Op2" | "Op3" => {
            assert!(!node.children.is_empty());
            let op = egraph.nodes.get(&node.children[0]).unwrap();

            if op.op.as_str() == "Reg" {
                if time == 0 {
                    let clk = egraph.nodes.get(&node.children[1]).unwrap();
                    let InterpreterResult::Bitvector(curr_clk_val, _) =
                        interpret_helper(egraph, &clk.eclass, time, env, cache).unwrap();
                    assert_eq!(
                        curr_clk_val, 0,
                        "We don't currently know what to do when clk=1 at time 0! See #88"
                    );
                    let initial_value = egraph.nodes.get(&op.children[0]).unwrap();
                    return Ok(InterpreterResult::Bitvector(
                        initial_value.op.parse().unwrap(),
                        get_bitwidth_for_node(egraph, &node.children[2]).unwrap(),
                    ));
                } else {
                    let clk = egraph.nodes.get(&node.children[1]).unwrap();
                    let InterpreterResult::Bitvector(prev_clk_val, _) =
                        interpret_helper(egraph, &clk.eclass, time - 1, env, cache).unwrap();
                    let InterpreterResult::Bitvector(curr_clk_val, _) =
                        interpret_helper(egraph, &clk.eclass, time, env, cache).unwrap();

                    if prev_clk_val == 0 && curr_clk_val == 1 {
                        let d = egraph.nodes.get(&node.children[2]).unwrap();
                        return interpret_helper(egraph, &d.eclass, time - 1, env, cache);
                    } else {
                        return interpret_helper(egraph, id, time - 1, env, cache);
                    }
                }
            }
            let children: Vec<_> = node
                .children
                .iter()
                .skip(1)
                .map(|id| {
                    let child = egraph.nodes.get(id).unwrap();
                    interpret_helper(egraph, &child.eclass, time, env, cache)
                })
                .collect();

            match op.op.as_str() {
                // Binary operations that condense to a single bit.
                "Eq" | "LogicOr" | "LogicAnd" | "Ne" => {
                    assert_eq!(children.len(), 2);
                    let result = match op.op.as_str() {
                        "Eq" => {
                            let a = match &children[0] {
                                Ok(InterpreterResult::Bitvector(val, _)) => *val,
                                _ => todo!(),
                            };
                            let b = match &children[1] {
                                Ok(InterpreterResult::Bitvector(val, _)) => *val,
                                _ => todo!(),
                            };
                            a == b
                        }
                        "Ne" => {
                            let a = match &children[0] {
                                Ok(InterpreterResult::Bitvector(val, _)) => *val,
                                _ => todo!(),
                            };
                            let b = match &children[1] {
                                Ok(InterpreterResult::Bitvector(val, _)) => *val,
                                _ => todo!(),
                            };
                            a != b
                        }
                        "LogicOr" => {
                            let result = children.iter().any(|child| match child {
                                Ok(InterpreterResult::Bitvector(val, _)) => *val != 0,
                                _ => todo!(),
                            });
                            result
                        }
                        "LogicAnd" => {
                            // if any of the children are false, the result is false
                            let result = children.iter().all(|child| match child {
                                Ok(InterpreterResult::Bitvector(val, _)) => *val != 0,
                                _ => todo!(),
                            });
                            result
                        }
                        _ => todo!(),
                    };
                    Ok(InterpreterResult::Bitvector(result as u64, 1))
                }
                // Unary operations that condense to a single bit.
                "ReduceOr" | "ReduceAnd" | "LogicNot" => {
                    assert_eq!(children.len(), 1);
                    match op.op.as_str() {
                        "ReduceOr" => {
                            let value = match children[0] {
                                Ok(InterpreterResult::Bitvector(val, _)) => val,
                                _ => todo!(),
                            };
                            let result = value != 0;
                            Ok(InterpreterResult::Bitvector(result as u64, 1))
                        }
                        "ReduceAnd" => {
                            // if any bit of children[0] is 0, the result is 0
                            match children[0] {
                                Ok(InterpreterResult::Bitvector(val, bw)) => {
                                    let result = val == (1 << bw) - 1;
                                    Ok(InterpreterResult::Bitvector(result as u64, 1))
                                }
                                _ => todo!(),
                            }
                        }
                        "LogicNot" => match children[0] {
                            Ok(InterpreterResult::Bitvector(val, _)) => {
                                let new_val = if val == 0 { 1 } else { 0 };
                                Ok(InterpreterResult::Bitvector(new_val, 1))
                            }
                            _ => todo!(),
                        },
                        _ => todo!(),
                    }
                }
                // Unary operations that preserve bitwidth.
                "Not" => {
                    assert_eq!(children.len(), 1);
                    match children[0] {
                        Ok(InterpreterResult::Bitvector(val, bw)) => {
                            let result = !val & ((1 << bw) - 1);
                            Ok(InterpreterResult::Bitvector(result, bw))
                        }
                        _ => todo!(),
                    }
                }
                // Binary operations that preserve bitwidth.
                "And" | "Or" | "Shr" | "Xor" | "Add" | "Sub" | "Mul" => {
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
                                "Xor" => a ^ b,
                                // TODO(@gussmith23): These might not work -- do we need to simulate lower bitwidths?
                                "Add" => (a.overflowing_add(*b).0) & ((1 << a_bw) - 1),
                                "Sub" => (a.overflowing_sub(*b).0) & ((1 << a_bw) - 1),
                                "Mul" => (a.overflowing_mul(*b).0) & ((1 << a_bw) - 1),
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
                            // TODO(@ninehusky): here, reading node.op.parse() as i64, then convert to u64
                            let val: i64 = node.op.parse().unwrap();
                            val as u64
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
                            assert!(
                                *bw > i && i >= j,
                                "i is {}, j is {} node has bw {}, has node_id {:?}",
                                i,
                                j,
                                bw,
                                node.children[1]
                            );

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
    };

    // Truncate. We do this in other places above, too, but this is a catch-all to ensure we don't forget.
    let result = match result {
        Ok(InterpreterResult::Bitvector(val, bw)) => Ok(InterpreterResult::Bitvector(
            truncate_value_to_bitwidth(val, bw),
            bw,
        )),
        _ => result,
    };

    if result.is_ok() {
        cache.insert((id.clone(), time), result.clone().unwrap());
    }
    result
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
            "output [{bw}-1:0] {name},\n",
            bw = get_bitwidth_for_node(egraph, &node.children[3]).unwrap(),
            name = egraph[&node.children[1]]
                .op
                .as_str()
                .strip_prefix('\"')
                .unwrap()
                .strip_suffix('\"')
                .unwrap()
        ));

        logic_declarations.push_str(&format!(
            "assign {name} = {wire};\n",
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
        let node_id = &choices[&id];
        let term = &egraph[node_id];

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
            "CRString" |
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
                    "CRString" => {
                        assert_eq!(op_node.children.len(), 1);
                        let value = &egraph[&op_node.children[0]].op;
                        logic_declarations.push_str(
                            // The string already has quotes in it. This may not
                            // always be the case. Should be careful here in the
                            // future.
                            format!(
                                // Localparam so that it's a constant.
                                "localparam [{}*8:0] {this_wire} = {value};\n",
                                value.len()-2,
                                this_wire = id_to_wire_name(&id),
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
                            // Localparam so that it's a constant.
                            "localparam [{bw}-1:0] {this_wire} = {bw}'d{value};\n",
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
                            "logic [{bw}-1:0] {this_wire} = {default};\n",
                            bw = get_bitwidth_for_node(egraph, node_id).unwrap(),
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
                    "Concat" | "Xor" |"And" | "Or" | "Mul" =>  {
                            assert_eq!(term.children.len(), 3, "Found {} op with #children = {}", op_node.op, term.children.len());
                    let expr0_id = &egraph[&term.children[1]].eclass;
                    let  expr1_id = &egraph[&term.children[2]].eclass;
                    logic_declarations.push_str(&format!(
                        "logic [{bw}-1:0] {this_wire} = {op};\n",
                        bw = get_bitwidth_for_node(egraph, node_id).unwrap(),
                        op = match op_node.op.as_str() {

                            "Concat" => format!("{{ {expr0}, {expr1} }}",
                        expr0 = id_to_wire_name(expr0_id),
                        expr1 = id_to_wire_name(expr1_id),
                        ),
                            "Xor" => format!("({expr0}^{expr1})",
                        expr0 = id_to_wire_name(expr0_id),
                        expr1 = id_to_wire_name(expr1_id),
                        ),
                            "And" => format!("({expr0}&{expr1})",
                        expr0 = id_to_wire_name(expr0_id),
                        expr1 = id_to_wire_name(expr1_id),
                        ),
                            "Or" => format!("({expr0}|{expr1})",
                        expr0 = id_to_wire_name(expr0_id),
                        expr1 = id_to_wire_name(expr1_id),
                        ),
                            "Mul" => format!("({expr0}*{expr1})",
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
                        "logic [{bw}-1:0] {this_wire} = {expr}[{hi}:{lo}];\n",
                        bw = get_bitwidth_for_node(egraph, node_id).unwrap(),
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
                        "logic [{bw}-1:0] {this_wire};\n",
                        bw = get_bitwidth_for_node(egraph, node_id).unwrap(),
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
        .parse_and_run_program(&format!(
            r#"(include "{}/egglog_src/churchroad.egg")"#,
            std::env::var("CARGO_MANIFEST_DIR").unwrap()
        ))
        .unwrap();

    // STEP 2: add the `debruijnify` primitive to the egraph. This depends on
    // the above language definitions, but it's not possible to do it in egglog,
    // hence it's a Rust function.
    add_debruijnify(egraph);

    // STEP 3: import module enumeration rewrites. These depend on the
    // `debruijnify` primitive.
    egraph
        .parse_and_run_program(&format!(
            r#"(include "{}/egglog_src/module_enumeration_rewrites.egg")"#,
            std::env::var("CARGO_MANIFEST_DIR").unwrap()
        ))
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
            let serialized = egraph.serialize_for_graphviz(true, usize::MAX, usize::MAX);
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
  logic out = wire_10;
  logic wire_10 = 0;
  
always @(posedge clk) begin
                            wire_10 <= wire_10;
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
  logic out = wire_27;
  logic wire_27;
  logic [4-1:0] wire_19 = 4'd4;
  logic [8-1:0] wire_13 = b;
  logic [8-1:0] wire_10 = a;
  

  some_module #(
    .p(wire_19)
) module_26 (
    .a(wire_10),
    .b(wire_13),
    .out(wire_27));
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
}
