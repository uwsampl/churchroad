// This file contains tests for the interpreter module.

use std::{fmt::Write, fs, io::Write as IOWrite, path::PathBuf, vec};

use egraph_serialize::NodeId;
use rand::{rngs::StdRng, RngCore, SeedableRng};

use egglog::{EGraph, SerializeConfig};

use churchroad::{get_bitwidth_for_node, import_churchroad, interpret, InterpreterResult};

// Creates an EGraph from a Verilog file using Churchroad, and returns the serialized EGraph and the root node.
fn prep_interpreter(
    module_verilog_path: PathBuf,
    test_output_dir: PathBuf,
    top_module_name: &str,
    out: &str,
) -> (egraph_serialize::EGraph, egraph_serialize::Node) {
    if std::env::var("CHURCHROAD_DIR").is_err() {
        panic!("Please set the CHURCHROAD_DIR environment variable!");
    }

    let churchroad_dir_str: String = std::env::var("CHURCHROAD_DIR").unwrap();

    let churchroad_yosys_dir =
        std::path::Path::new(&churchroad_dir_str).join("yosys-plugin/churchroad.so");

    let churchroad_src_path = test_output_dir.join(format!("{}.egg", top_module_name));

    let yosys_commands = format!(
        "read_verilog -sv {}; prep -top {}; pmuxtree; write_churchroad",
        module_verilog_path.to_str().unwrap(),
        top_module_name,
    );

    let yosys_proc = std::process::Command::new("yosys")
        .arg("-m")
        .arg(churchroad_yosys_dir)
        .arg("-q")
        .arg("-p")
        .arg(yosys_commands)
        .output();

    let yosys_output = yosys_proc.unwrap();

    if !yosys_output.status.success() {
        panic!(
            "Yosys failed, stderr: {:?}",
            String::from_utf8(yosys_output.stderr)
        );
    }

    std::fs::write(&churchroad_src_path, yosys_output.stdout).unwrap();
    println!(
        "logged Churchroad source to: {}",
        churchroad_src_path.to_str().unwrap()
    );

    // prep egraph for interpretation
    let mut egraph: EGraph = EGraph::default();
    import_churchroad(&mut egraph);
    egraph
        .parse_and_run_program(&std::fs::read_to_string(churchroad_src_path).unwrap())
        .unwrap();

    egraph
        .parse_and_run_program("(run-schedule (saturate typing))")
        .unwrap();

    let serialized = egraph.serialize(SerializeConfig::default());

    let (_, is_output_node) = serialized
        .nodes
        .iter()
        .find(|(_, n)| {
            n.op == "IsPort"
                && n.children[2] == NodeId::from("Output-0")
                && serialized.nodes.get(&n.children[1]).unwrap().op.as_str()
                    == format!("\"{}\"", out)
        })
        .unwrap();

    // output the serialized egraph to "DSP48E2.json"
    serialized
        .to_json_file(test_output_dir.join("serialized.json"))
        .unwrap();

    println!(
        "Logged JSON to: {}",
        test_output_dir.join("serialized.json").to_str().unwrap()
    );

    // Each node should have a HasType node associated with it.
    for (node_id, node) in serialized.nodes.iter() {
        // if the node has op in the list, don't care
        let list = ["Var", "Op1"];
        if !list.contains(&node.op.as_str()) {
            continue;
        }
        let _ = get_bitwidth_for_node(&serialized, node_id);
    }

    let output_id = is_output_node.children.last().unwrap();
    let (_, output_node) = serialized
        .nodes
        .iter()
        .find(|(node_id, _)| **node_id == *output_id)
        .unwrap();

    (serialized.clone(), output_node.clone())
}

// TODO(@ninehusky): macroify this
#[test]
fn test_lut6_combinational_verilator() {
    if std::env::var("CHURCHROAD_DIR").is_err() {
        panic!("Please set the CHURCHROAD_DIR environment variable!");
    }
    let churchroad_dir_str: String = std::env::var("CHURCHROAD_DIR").unwrap();
    let churchroad_dir = std::path::Path::new(&churchroad_dir_str);
    let testbench_template_path =
        churchroad_dir.join("tests/interpreter_tests/verilog/testbench.sv.template");

    let inputs = vec![
        ("INIT", 64),
        ("I0", 1),
        ("I1", 1),
        ("I2", 1),
        ("I3", 1),
        ("I4", 1),
        ("I5", 1),
    ];
    let outputs: Vec<(&str, i32)> = vec![("O", 1)];

    let include_dirs =
        vec![churchroad_dir.join("tests/interpreter_tests/verilog/xilinx_ultrascale_plus/")];

    verilator_vs_interpreter(
        100,
        5,
        testbench_template_path,
        "LUT6",
        inputs,
        outputs,
        include_dirs,
        std::env::temp_dir(),
        churchroad_dir
            .join("tests/interpreter_tests/verilog/xilinx_ultrascale_plus/LUT6-modified.v"),
    );
}

// TODO(@ninehusky): macroify this
#[should_panic = "assertion `left == right` failed: We don't currently know what to do when clk=1 at time 0! See #88"]
#[test]
fn test_counter_verilator() {
    if std::env::var("CHURCHROAD_DIR").is_err() {
        panic!("Please set the CHURCHROAD_DIR environment variable!");
    }
    let churchroad_dir_str: String = std::env::var("CHURCHROAD_DIR").unwrap();
    let churchroad_dir = std::path::Path::new(&churchroad_dir_str);
    let testbench_template_path =
        churchroad_dir.join("tests/interpreter_tests/verilog/testbench.sv.template");

    let inputs = vec![("clk", 1)];
    let outputs = vec![("count", 4)];

    let include_dirs = vec![
        churchroad_dir.join("tests/interpreter_tests/verilog/"),
        churchroad_dir.join("tests/interpreter_tests/verilog/toy_examples/"),
    ];

    verilator_vs_interpreter(
        3,
        10,
        testbench_template_path,
        "counter",
        inputs,
        outputs,
        include_dirs,
        std::env::temp_dir(),
        churchroad_dir.join("tests/interpreter_tests/verilog/toy_examples/counter.sv"),
    );
}

fn verilator_vs_interpreter(
    num_test_cases: usize,
    num_clock_cycles: usize,
    testbench_template_path: PathBuf,
    top_module_name: &str,
    inputs: Vec<(&str, i32)>,
    outputs: Vec<(&str, i32)>,
    include_dirs: Vec<PathBuf>,
    test_output_dir: PathBuf,
    verilog_module_path: PathBuf,
) {
    // create seeded rng
    let mut rng = StdRng::seed_from_u64(0xb0bacafe);
    let mut interpreter_results: Vec<InterpreterResult> = Vec::new();
    let test_vectors: Vec<Vec<Vec<u64>>> = (0..num_test_cases)
        .map(|_| {
            (0..num_clock_cycles)
                .map(|_| {
                    inputs
                        .iter()
                        .map(|(_, bw)| {
                            assert!(*bw <= 64);
                            rng.next_u64()
                                & ((1u64.checked_shl((*bw).try_into().unwrap()).unwrap_or(0))
                                    .wrapping_sub(1))
                        })
                        .collect()
                })
                .collect()
        })
        .collect();

    let (serialized, root_node) = prep_interpreter(
        verilog_module_path.clone(),
        test_output_dir.clone(),
        top_module_name,
        outputs[0].0,
    );

    // Interpret all test vectors.
    for test_case in test_vectors.iter() {
        let env = inputs
            .iter()
            .enumerate()
            .map(|(input_idx, (name, _))| {
                (
                    name.to_owned(),
                    test_case
                        .iter()
                        .map(|vals_at_timestep| vals_at_timestep[input_idx])
                        .collect(),
                )
            })
            .collect();

        // TODO(@gussmith23): This is inefficient. Either the interpreter should
        // return streams, or we should be able to memoize some way. This just
        // redoes a bunch of work each call.
        for timestep in 0..num_clock_cycles {
            let result = interpret(&serialized, &root_node.eclass, timestep, &env).unwrap();
            interpreter_results.push(result);
        }
    }

    let verilator_output_values: Vec<u64> = run_verilator(
        testbench_template_path,
        top_module_name,
        inputs,
        outputs,
        test_vectors,
        include_dirs,
        test_output_dir,
        verilog_module_path,
    );

    // let test_output_path = test_output_dir.join("test_output.txt");
    // let test_error_path = test_output_dir.join("test_error.txt");

    // std::fs::write(&test_output_path, output_str).unwrap();
    // std::fs::write(&test_error_path, output.stderr).unwrap();

    assert_eq!(interpreter_results.len(), verilator_output_values.len());

    for (InterpreterResult::Bitvector(val, _), verilator_result) in interpreter_results
        .iter()
        .zip(verilator_output_values.iter())
    {
        assert_eq!(val, verilator_result);
    }

    // println!("logged output to: {}", test_output_path.to_str().unwrap());
}

// This test runs verilator against our interpreter, failing if the outputs of the two differ.
//
// testbench_template_path: path to the testbench template file
// top_module_name: name of the top module in the Verilog file
// inputs: list of tuples of input names and their bitwidths
// outputs: list of tuples of output names and their bitwidths
// include_dirs: list of directories to include in the Verilator compilation
// test_output_dir: directory to output test files to
// churchroad_src_path: path to the Churchroad source file that the interpreter will use
// num_test_cases: number of test cases to run
// num_clock_cycles: number of clock cycles to run each test case for
// filename: name of the file to run Verilator on. For example, if the file is `adder.v`, this should be `"adder.v"`
// test_vectors: A vector of vectors of vectors. the ith entry in the outermost
//   vector contains the inputs for the ith test case. The jth entry in the ith
//   test case are the inputs at clock cycle j. The kth entry in the jth set of
//   inputs is the value of the kth input at clock cycle j, where the inputs are
//   ordered as they appear in the inputs vector.
fn run_verilator(
    testbench_template_path: PathBuf,
    top_module_name: &str,
    inputs: Vec<(&str, i32)>,
    outputs: Vec<(&str, i32)>,
    test_vectors: Vec<Vec<Vec<u64>>>,
    include_dirs: Vec<PathBuf>,
    test_output_dir: PathBuf,
    verilog_module_path: PathBuf,
) -> Vec<u64> {
    let testbench_path = test_output_dir.join("testbench.sv");

    // just grab the filename without any leading directories
    let filename = verilog_module_path.file_name().unwrap().to_str().unwrap();

    let test_module_input_list = {
        let inputs_expr = inputs
            .iter()
            .enumerate()
            .map(|(i, (name, _))| format!(".{}(inputs[{}])", name, i))
            .collect::<Vec<String>>()
            .join(", ")
            .to_string();
        let outputs_expr = outputs
            .iter()
            .map(|(name, _)| format!(".{}({})", name, name))
            .collect::<Vec<String>>()
            .join(", ")
            .to_string();
        let mut final_expr = String::from("");
        if !inputs_expr.is_empty() {
            final_expr.push_str(inputs_expr.as_str());
        }
        if !outputs_expr.is_empty() {
            final_expr.push_str(", ");
            final_expr.push_str(outputs_expr.as_str());
        };
        final_expr
    };

    let testbench_prog = std::fs::read_to_string(testbench_template_path)
        .unwrap()
        .replace("{filename}", filename)
        // TODO(@ninehusky): this'll eventually need to include parameters as well, right?
        .replace(
            "{input_output_declarations}",
            outputs
                .iter()
                .map(|(name, bw)| format!("logic [{}:0] {};\n", bw - 1, name))
                .collect::<Vec<String>>()
                .join("\n")
                .to_string()
                .as_str(),
        )
        .replace("{test_module_name}", top_module_name)
        .replace("{test_module_port_list}", test_module_input_list.as_str())
        .replace(
            "{max_input_bitwidth}",
            inputs
                .iter()
                .map(|(_, bitwidth)| bitwidth)
                .max()
                .unwrap_or(&1)
                .to_string()
                .as_str(),
        )
        .replace(
            "{display_inputs}",
            inputs
                .iter()
                .map(|(name, _)| {
                    format!(
                        "$display(\"inputs: {} %d\\n\", simulate_with_verilator_test_module.{});",
                        name, name
                    )
                })
                .collect::<Vec<String>>()
                .join("\n")
                .as_str(),
        )
        .replace(
            "{display_outputs}",
            outputs
                .iter()
                .map(|(name, _)| {
                    format!(
                        "$display(\"output: %d\\n\", simulate_with_verilator_test_module.{});",
                        name
                    )
                })
                .collect::<Vec<String>>()
                .join("\n")
                .as_str(),
        );
    // .replace("{set_clock}", set_clock_expr.as_str())
    // .replace("{clear_clock}", clear_clock_expr.as_str());

    let executable_name = "executable";
    let verilator_output_dir = test_output_dir.join("obj_dir");
    let executable_path = verilator_output_dir.join(executable_name);

    std::fs::write(&testbench_path, testbench_prog).unwrap();

    // TODO(@ninehusky): We can get rid of the necessity for a Makefile after this PR is merged
    // into Verilator: https://github.com/verilator/verilator/pull/5031
    let verilator_compile_output = std::process::Command::new("verilator")
        .arg("-o")
        .arg(executable_name)
        .arg("-Wno-WIDTHTRUNC")
        .arg("--assert")
        .arg("--timing")
        .arg("--binary")
        .arg("--build")
        .arg("--Mdir")
        .arg(&verilator_output_dir)
        .args(
            include_dirs
                .iter()
                .map(|path| format!("-I{}", path.to_str().unwrap())),
        )
        .arg(testbench_path.to_str().unwrap())
        .output()
        .unwrap();

    if !verilator_compile_output.status.success() {
        panic!(
            "Verilator failed, stderr: {:?}",
            String::from_utf8(verilator_compile_output.stderr)
        );
    }

    let mut sim_proc = std::process::Command::new(executable_path)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();

    let num_inputs = inputs.len();

    let num_test_cases = test_vectors.len();
    let num_clock_cycles = if num_test_cases > 0 {
        test_vectors[0].len()
    } else {
        0
    };
    assert!(test_vectors.iter().all(|v| v.len() == num_clock_cycles));
    assert!(test_vectors
        .iter()
        .all(|v| v.iter().all(|inputs| inputs.len() == num_inputs)));

    let mut inputs_str = String::new();
    inputs_str
        .write_str(&format!(
            "{} {} {}\n",
            num_inputs, num_test_cases, num_clock_cycles
        ))
        .unwrap();

    for test_case in test_vectors.iter() {
        for inputs in test_case.iter() {
            for input in inputs.iter() {
                inputs_str.write_str(&format!("{:X}\n", input)).unwrap();
            }
        }
    }

    sim_proc
        .stdin
        .as_mut()
        .unwrap()
        .write_all(inputs_str.as_bytes())
        .unwrap();

    fs::write(test_output_dir.join("inputs.txt"), inputs_str).unwrap();

    let output = sim_proc.wait_with_output().unwrap();
    let output_str = String::from_utf8(output.stdout).unwrap();
    let verilator_output_values: Vec<u64> = output_str
        .lines()
        // filter all lines that don't start with "output: "
        .filter(|line| !line.is_empty() && line.starts_with("output: "))
        .map(|line| line.trim_start_matches("output: ").parse().unwrap())
        .collect();

    fs::write(test_output_dir.join("output.txt"), output_str).unwrap();

    verilator_output_values
}

macro_rules! interpreter_test_verilog {
    ($(#[$meta:meta])* $test_name:ident, $expected:expr, $verilog_path:literal, $module_name:literal, $time:literal, $env:expr, $out: literal) => {
        $(#[$meta])*
        #[test]
        fn $test_name() {
            let (serialized, root_node) = prep_interpreter(
                PathBuf::from($verilog_path),
                std::env::temp_dir(),
                $module_name,
                $out,
            );

            assert_eq!(
                $expected,
                interpret(&serialized, &root_node.eclass, $time, $env).unwrap()
            );
        }
    };
}

macro_rules! interpreter_test_churchroad {
    ($test_name:ident, $churchroad_src:literal, $time:literal, $out:expr, $env:expr, $expected:expr) => {
        #[test]
        fn $test_name() {
            let mut egraph: EGraph = EGraph::default();

            import_churchroad(&mut egraph);
            egraph.parse_and_run_program($churchroad_src).unwrap();

            egraph
                .parse_and_run_program("(run-schedule (saturate typing))")
                .unwrap();

            let serialized = egraph.serialize(SerializeConfig::default());

            let (_, is_output_node) = serialized
                .nodes
                .iter()
                .find(|(_, n)| {
                    n.op == "IsPort"
                        && n.children[2] == NodeId::from("Output-0")
                        && serialized.nodes.get(&n.children[1]).unwrap().op.as_str()
                            == format!("\"{}\"", $out)
                })
                .unwrap();

            let output_id = is_output_node.children.last().unwrap();
            let (_, output_node) = serialized
                .nodes
                .iter()
                .find(|(node_id, _)| **node_id == *output_id)
                .unwrap();

            let interpreter_result =
                interpret(&serialized, &output_node.eclass, $time, $env).unwrap();
            assert_eq!(
                $expected, interpreter_result,
                "(left: expected, right: interpreter_result)"
            );
        }
    };
}

interpreter_test_churchroad!(
    add_single_operation,
    r#"
    (let v0 (Var "a" 4))
    (let v1 (Var "b" 4))
    (let v2 (Op2 (Add) v0 v1))
    (IsPort "" "v2" (Output) v2)
    "#,
    0,
    "v2",
    &[("a", vec![0b1000]), ("b", vec![0b0001])].into(),
    InterpreterResult::Bitvector(9, 4)
);

interpreter_test_churchroad!(
    sub_single_operation,
    r#"
    (let v0 (Var "a" 8))
    (let v1 (Var "b" 8))
    (let v2 (Op2 (Sub) v0 v1))
    (IsPort "" "v2" (Output) v2)
    "#,
    0,
    "v2",
    &[("a", vec![0b11111111]), ("b", vec![0b00000001])].into(),
    InterpreterResult::Bitvector(0b11111110, 8)
);

interpreter_test_churchroad!(
    mul_single_operation,
    r#"
    (let v0 (Var "a" 8))
    (let v1 (Var "b" 8))
    (let v2 (Op2 (Mul) v0 v1))
    (IsPort "" "v2" (Output) v2)
    "#,
    0,
    "v2",
    &[("a", vec![2]), ("b", vec![3])].into(),
    InterpreterResult::Bitvector(6, 8)
);

interpreter_test_churchroad!(
    or_single_operation,
    r#"
    (let v0 (Var "a" 8))
    (let v1 (Var "b" 8))
    (let v2 (Op2 (Or) v0 v1))
    (IsPort "" "v2" (Output) v2)
    "#,
    0,
    "v2",
    &[("a", vec![0b10101010]), ("b", vec![0b01010100])].into(),
    InterpreterResult::Bitvector(0b11111110, 8)
);

interpreter_test_churchroad!(
    xor_single_operation,
    r#"
    (let v0 (Var "a" 8))
    (let v1 (Var "b" 8))
    (let v2 (Op2 (Xor) v0 v1))
    (IsPort "" "v2" (Output) v2)
    "#,
    0,
    "v2",
    &[("a", vec![0b10101010]), ("b", vec![0b01010110])].into(),
    InterpreterResult::Bitvector(0b11111100, 8)
);

interpreter_test_churchroad!(
    shr_single_operation,
    r#"
    (let v0 (Var "a" 8))
    (let v1 (Var "b" 8))
    (let v2 (Op2 (Shr) v0 v1))
    (IsPort "" "v2" (Output) v2)
    "#,
    0,
    "v2",
    &[("a", vec![0b11101010]), ("b", vec![2])].into(),
    InterpreterResult::Bitvector(0b00111010, 8)
);

interpreter_test_churchroad!(
    eq_single_operation,
    r#"
    (let v0 (Var "a" 8))
    (let v1 (Var "b" 8))
    (let v2 (Op2 (Eq) v0 v1))
    (IsPort "" "v2" (Output) v2)
    "#,
    0,
    "v2",
    &[("a", vec![0b11101010]), ("b", vec![0b11101110])].into(),
    InterpreterResult::Bitvector(0, 1)
);

interpreter_test_churchroad!(
    not_single_operation,
    r#"
    (let v0 (Var "a" 8))
    (let v1 (Op1 (Not) v0))
    (IsPort "" "v1" (Output) v1)
    "#,
    0,
    "v1",
    &[("a", vec![0b11101010])].into(),
    InterpreterResult::Bitvector(0b00010101, 8)
);

interpreter_test_churchroad!(
    reduce_or_single_operation,
    r#"
    (let v0 (Var "a" 8))
    (let v1 (Op1 (ReduceOr) v0))
    (IsPort "" "v1" (Output) v1)
    "#,
    0,
    "v1",
    &[("a", vec![0b01000000])].into(),
    InterpreterResult::Bitvector(1, 1)
);

interpreter_test_churchroad!(
    reduce_and_single_operation,
    r#"
    (let v0 (Var "a" 8))
    (let v1 (Op1 (ReduceAnd) v0))
    (IsPort "" "v1" (Output) v1)
    "#,
    0,
    "v1",
    &[("a", vec![0b11111111])].into(),
    InterpreterResult::Bitvector(1, 1)
);

interpreter_test_churchroad!(
    logic_not_single_operation,
    r#"
    (let v0 (Var "a" 8))
    (let v1 (Op1 (LogicNot) v0))
    (IsPort "" "v1" (Output) v1)
    "#,
    0,
    "v1",
    &[("a", vec![0b11111101])].into(),
    InterpreterResult::Bitvector(0b0, 1)
);

interpreter_test_churchroad!(
    logic_and_single_operation,
    r#"
    (let v0 (Var "a" 8))
    (let v1 (Var "b" 8))
    (let v2 (Op2 (LogicAnd) v0 v1))
    (IsPort "" "v2" (Output) v2)
    "#,
    0,
    "v2",
    &[("a", vec![0b11101010]), ("b", vec![0b11101110])].into(),
    InterpreterResult::Bitvector(1, 1)
);

interpreter_test_churchroad!(
    logic_or_single_operation,
    r#"
    (let v0 (Var "a" 8))
    (let v1 (Var "b" 8))
    (let v2 (Op2 (LogicOr) v0 v1))
    (IsPort "" "v2" (Output) v2)
    "#,
    0,
    "v2",
    &[("a", vec![0b00000000]), ("b", vec![0b11101110])].into(),
    InterpreterResult::Bitvector(1, 1)
);

interpreter_test_churchroad!(
    extract_single_operation,
    r#"
    (let v0 (Var "a" 8))
    (let v1 (Op1 (Extract 4 2) v0))
    (IsPort "" "v1" (Output) v1)
    "#,
    0,
    "v1",
    &[("a", vec![0b101000])].into(),
    InterpreterResult::Bitvector(0b010, 3)
);

interpreter_test_churchroad!(
    concat_single_operation,
    r#"
    (let v0 (Var "a" 3))
    (let v1 (Var "b" 3))
    (let v2 (Op2 (Concat) v0 v1))
    (IsPort "" "v2" (Output) v2)
    "#,
    0,
    "v2",
    &[("a", vec![0b101]), ("b", vec![0b010])].into(),
    InterpreterResult::Bitvector(0b101010, 6)
);

interpreter_test_churchroad!(
    mux_single_operation,
    r#"
    (let v0 (Var "a" 1))
    (let v1 (Var "b" 8))
    (let v2 (Var "c" 8))
    (let v3 (Op3 (Mux) v0 v1 v2))
    (IsPort "" "v3" (Output) v3)
    "#,
    0,
    "v3",
    &[
        ("a", vec![1]),
        ("b", vec![0b10101010]),
        ("c", vec![0b01010101])
    ]
    .into(),
    InterpreterResult::Bitvector(0b01010101, 8)
);

interpreter_test_churchroad!(
    bv_single_operation,
    r#"
    (let v0 (Op0 (BV 8 8)))
    (IsPort "" "v0" (Output) v0)
    "#,
    0,
    "v0",
    &[].into(),
    InterpreterResult::Bitvector(8, 8)
);

interpreter_test_churchroad!(
    zeroextend_single_operation,
    r#"
    (let v0 (Var "a" 4))
    (let v1 (Op1 (ZeroExtend 8) v0))
    (IsPort "" "v1" (Output) v1)
    "#,
    0,
    "v1",
    &[("a", vec![0b1010])].into(),
    InterpreterResult::Bitvector(0b1010, 8)
);

interpreter_test_churchroad!(
    reg_single_operation_first_cycle,
    r#"
    (let v0 (Var "a" 8))
    (let clk (Var "clk" 1))
    (let v1 (Op2 (Reg 8) clk v0))
    (IsPort "" "v1" (Output) v1)
    "#,
    0,
    "v1",
    &[("a", vec![0b10101010]), ("clk", vec![0])].into(),
    InterpreterResult::Bitvector(8, 8)
);

interpreter_test_churchroad!(
    reg_single_operation_second_cycle,
    r#"
    (let v0 (Var "a" 8))
    (let clk (Var "clk" 1))
    (let v1 (Op2 (Reg 8) clk v0))
    (IsPort "" "v1" (Output) v1)
    "#,
    1,
    "v1",
    &[("a", vec![0b10101010, 0b0]), ("clk", vec![0, 1])].into(),
    InterpreterResult::Bitvector(0b10101010, 8)
);

interpreter_test_churchroad!(
    ne_single_operation,
    r#"
    (let v0 (Var "a" 8))
    (let v1 (Var "b" 8))
    (let v2 (Op2 (Ne) v0 v1))
    (IsPort "" "v2" (Output) v2)
    "#,
    0,
    "v2",
    &[("a", vec![0b11101010]), ("b", vec![0b11101110])].into(),
    InterpreterResult::Bitvector(1, 1)
);

interpreter_test_verilog!(
    simple_mux_0,
    InterpreterResult::Bitvector(1, 1),
    "tests/interpreter_tests/verilog/toy_examples/simple_mux.sv",
    "simple_mux",
    0,
    &[("a", vec![0]), ("b", vec![0]), ("c", vec![1])].into(),
    "o"
);

interpreter_test_verilog!(
    simple_mux_1,
    InterpreterResult::Bitvector(0, 1),
    "tests/interpreter_tests/verilog/toy_examples/simple_mux.sv",
    "simple_mux",
    0,
    &[("a", vec![1]), ("b", vec![0]), ("c", vec![1])].into(),
    "o"
);

interpreter_test_verilog!(
    test_alu_and_single_cycle,
    InterpreterResult::Bitvector(0b01010101, 8),
    "tests/interpreter_tests/verilog/toy_examples/ALU.sv",
    "ALU",
    0,
    &[
        ("a", vec![0b01010101]),
        ("b", vec![0b11111111]),
        ("op", vec![1])
    ]
    .into(),
    "out"
);

interpreter_test_verilog!(
    test_alu_and_single_cycle_2,
    InterpreterResult::Bitvector(0b01010101, 8),
    "tests/interpreter_tests/verilog/toy_examples/ALU.sv",
    "ALU",
    0,
    &[
        ("a", vec![0b01010101, 0b10101010]),
        ("b", vec![0b11111111, 0b00000000]),
        ("op", vec![1, 0])
    ]
    .into(),
    "out"
);

interpreter_test_verilog!(
    test_alu_or_second_cycle,
    InterpreterResult::Bitvector(0b10101010, 8),
    "tests/interpreter_tests/verilog/toy_examples/ALU.sv",
    "ALU",
    1,
    &[
        ("a", vec![0b01010101, 0b10101010]),
        ("b", vec![0b11111111, 0b00000000]),
        ("op", vec![1, 0])
    ]
    .into(),
    "out"
);

interpreter_test_verilog!(
    test_counter_first_cycle,
    InterpreterResult::Bitvector(2, 4),
    "tests/interpreter_tests/verilog/toy_examples/counter.sv",
    "counter",
    5,
    &[("clk", vec![0, 0, 1, 1, 0, 1])].into(),
    "count"
);

interpreter_test_verilog!(
    test_counter_third_cycle,
    InterpreterResult::Bitvector(3, 4),
    "tests/interpreter_tests/verilog/toy_examples/counter.sv",
    "counter",
    6,
    &[("clk", vec![0, 1, 0, 1, 0, 1, 0])].into(),
    "count"
);

interpreter_test_verilog!(
    #[should_panic]
    dummy_dsp_test,
    InterpreterResult::Bitvector(2, 48),
    "tests/interpreter_tests/verilog/xilinx_ultrascale_plus/DSP48E2.v",
    "DSP48E2",
    0,
    &[
        // The following is the configuration for a DSP48E2 that is an "addmuland" in 0 stages
        ("ACASCREG", vec![0]),
        ("ADREG", vec![0]),
        ("ALUMODEREG", vec![0]),
        ("AMULTSEL", vec![2]),
        ("AREG", vec![0]),
        ("AUTORESET_PATDET", vec![3]),
        ("AUTORESET_PRIORITY", vec![4]),
        ("A_INPUT", vec![7]),
        ("BCASCREG", vec![0]),
        ("BMULTSEL", vec![1]),
        ("BREG", vec![(0)]),
        ("B_INPUT", vec![7]),
        ("CARRYINREG", vec![0]),
        ("CARRYINSELREG", vec![0]),
        ("CREG", vec![0]),
        ("DREG", vec![0]),
        ("INMODEREG", vec![0]),
        ("IS_ALUMODE_INVERTED", vec![0]),
        ("IS_CARRYIN_INVERTED", vec![0]),
        ("IS_CLK_INVERTED", vec![0]),
        ("IS_INMODE_INVERTED", vec![0]),
        ("IS_OPMODE_INVERTED", vec![0]),
        ("IS_RSTALLCARRYIN_INVERTED", vec![0]),
        ("IS_RSTALUMODE_INVERTED", vec![0]),
        ("IS_RSTA_INVERTED", vec![0]),
        ("IS_RSTB_INVERTED", vec![0]),
        ("IS_RSTCTRL_INVERTED", vec![0]),
        ("IS_RSTC_INVERTED", vec![0]),
        ("IS_RSTD_INVERTED", vec![0]),
        ("IS_RSTINMODE_INVERTED", vec![0]),
        ("IS_RSTM_INVERTED", vec![0]),
        ("IS_RSTP_INVERTED", vec![0]),
        ("MASK", vec![0]),
        ("MREG", vec![0]),
        ("OPMODEREG", vec![0]),
        ("PATTERN", vec![0]),
        ("PREADDINSEL", vec![0]),
        ("PREG", vec![0]),
        ("RND", vec![0]),
        ("SEL_MASK", vec![8]),
        ("SEL_PATTERN", vec![9]),
        ("USE_MULT", vec![10]),
        ("USE_PATTERN_DETECT", vec![11]),
        ("USE_SIMD", vec![12]),
        ("USE_WIDEXOR", vec![13]),
        ("XORSIMD", vec![26]),
        // ("A", vec![({ a[7], a[7], a[7], a[7], a[7], a[7], a[7], a[7], a[7], a[7], a[7], a[7], a[7], a[7], a[7], a[7], a[7], a[7], a[7], a[7], a[7], a[7], a })]),
        ("A", vec![2]),
        ("ACIN", vec![0]),
        ("ALUMODE", vec![3]),
        // ("B", vec![({ 10'h000, b })]),
        ("B", vec![1]),
        ("BCIN", vec![0]),
        // ("C", vec![({ c[7], c[7], c[7], c[7], c[7], c[7], c[7], c[7], c[7], c[7], c[7], c[7], c[7], c[7], c[7], c[7], c[7], c[7], c[7], c[7], c[7], c[7], c[7], c[7], c[7], c[7], c[7], c[7], c[7], c[7], c[7], c[7], c[7], c[7], c[7], c[7], c[7], c[7], c[7], c[7], c })]),
        ("C", vec![0]),
        ("CARRYCASCIN", vec![0]),
        ("CARRYIN", vec![0]),
        ("CARRYINSEL", vec![0]),
        ("CEA1", vec![1]),
        ("CEA2", vec![1]),
        ("CEAD", vec![1]),
        ("CEALUMODE", vec![1]),
        ("CEB1", vec![1]),
        ("CEB2", vec![1]),
        ("CEC", vec![1]),
        ("CECARRYIN", vec![1]),
        ("CECTRL", vec![1]),
        ("CED", vec![1]),
        ("CEINMODE", vec![1]),
        ("CEM", vec![1]),
        ("CEP", vec![1]),
        ("CLK", vec![0]),
        // ("D", vec![({ c[7], c[7], c[7], c[7], c[7], c[7], c[7], c[7], c[7], c[7], c[7], c[7], c[7], c[7], c[7], c[7], c[7], c[7], c[7], c })]),
        ("D", vec![0]),
        ("INMODE", vec![8]),
        ("MULTSIGNIN", vec![0]),
        ("OPMODE", vec![0b000100110101]),
        // ("P", vec![({ P_0[47:8], out })]),
        ("PCIN", vec![0]),
        ("RSTA", vec![0]),
        ("RSTALLCARRYIN", vec![0]),
        ("RSTALUMODE", vec![0]),
        ("RSTB", vec![0]),
        ("RSTC", vec![0]),
        ("RSTCTRL", vec![0]),
        ("RSTD", vec![0]),
        ("RSTINMODE", vec![0]),
        ("RSTM", vec![0]),
        ("RSTP", vec![0])
    ]
    .into(),
    "P"
);
#[test]
fn test_run_verilator() {
    if std::env::var("CHURCHROAD_DIR").is_err() {
        panic!("Please set the CHURCHROAD_DIR environment variable!");
    }
    let churchroad_dir_str: String = std::env::var("CHURCHROAD_DIR").unwrap();
    let churchroad_dir = std::path::Path::new(&churchroad_dir_str);
    let testbench_template_path =
        churchroad_dir.join("tests/interpreter_tests/verilog/testbench.sv.template");

    let inputs = vec![
        ("INIT", 64),
        ("I0", 1),
        ("I1", 1),
        ("I2", 1),
        ("I3", 1),
        ("I4", 1),
        ("I5", 1),
    ];
    let outputs: Vec<(&str, i32)> = vec![("O", 1)];

    let include_dirs =
        vec![churchroad_dir.join("tests/interpreter_tests/verilog/xilinx_ultrascale_plus")];

    assert_eq!(
        run_verilator(
            testbench_template_path.clone(),
            "LUT6",
            inputs.clone(),
            outputs.clone(),
            vec![vec![vec![0, 0, 0, 0, 0, 0, 0]]],
            include_dirs.clone(),
            std::env::temp_dir(),
            churchroad_dir
                .join("tests/interpreter_tests/verilog/xilinx_ultrascale_plus/LUT6-modified.v"),
        ),
        vec![0]
    );

    assert_eq!(
        run_verilator(
            testbench_template_path.clone(),
            "LUT6",
            inputs.clone(),
            outputs.clone(),
            vec![vec![vec![0xFFFFFFFFFFFFFFFF, 1, 0, 0, 0, 0, 0]]],
            include_dirs.clone(),
            std::env::temp_dir(),
            churchroad_dir.join("tests/interpreter_tests/verilog/LUT6-modified.v"),
        ),
        vec![1]
    );

    assert_eq!(
        run_verilator(
            testbench_template_path.clone(),
            "LUT6",
            inputs.clone(),
            outputs.clone(),
            vec![vec![vec![0b10, 1, 0, 0, 0, 0, 0]]],
            include_dirs.clone(),
            std::env::temp_dir(),
            churchroad_dir.join("tests/interpreter_tests/verilog/LUT6-modified.v"),
        ),
        vec![1]
    );

    assert_eq!(
        run_verilator(
            testbench_template_path.clone(),
            "LUT6",
            inputs.clone(),
            outputs.clone(),
            vec![vec![vec![0b000001000000000000, 0, 0, 1, 1, 0, 0]]],
            include_dirs.clone(),
            std::env::temp_dir(),
            churchroad_dir.join("tests/interpreter_tests/verilog/LUT6-modified.v"),
        ),
        vec![1]
    );

    assert_eq!(
        run_verilator(
            testbench_template_path.clone(),
            "LUT6",
            inputs.clone(),
            outputs.clone(),
            vec![vec![vec![0b1000000000000, 1, 0, 1, 1, 0, 0]]],
            include_dirs.clone(),
            std::env::temp_dir(),
            churchroad_dir.join("tests/interpreter_tests/verilog/LUT6-modified.v"),
        ),
        vec![0]
    );

    assert_eq!(
        run_verilator(
            testbench_template_path.clone(),
            "LUT6",
            inputs.clone(),
            outputs.clone(),
            vec![vec![
                vec![0b1000000000000, 1, 0, 1, 1, 0, 0],
                vec![0b1000000000000, 0, 0, 1, 1, 0, 0],
                vec![0b0100000000000, 0, 0, 1, 1, 0, 0],
            ]],
            include_dirs.clone(),
            std::env::temp_dir(),
            churchroad_dir.join("tests/interpreter_tests/verilog/LUT6-modified.v"),
        ),
        vec![0, 1, 0]
    );
}
