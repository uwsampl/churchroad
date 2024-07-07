// This file contains tests for the interpreter module.

use std::{fmt::Write, fs, io::Write as IOWrite, path::PathBuf, vec};

use egraph_serialize::NodeId;
use rand::{rngs::StdRng, RngCore, SeedableRng};

use egglog::{EGraph, SerializeConfig};

use churchroad::{import_churchroad, interpret, InterpreterResult};

// Creates an EGraph from a Verilog file using Churchroad, and returns the serialized EGraph and the root node.
fn prep_interpreter(
    module_verilog_path: PathBuf,
    test_output_dir: PathBuf,
    top_module_name: &str,
) -> (egraph_serialize::EGraph, egraph_serialize::Node) {
    if std::env::var("CHURCHROAD_DIR").is_err() {
        panic!("Please set the CHURCHROAD_DIR environment variable!");
    }

    let churchroad_dir_str: String = std::env::var("CHURCHROAD_DIR").unwrap();

    let churchroad_yosys_dir =
        std::path::Path::new(&churchroad_dir_str).join("yosys-plugin/churchroad.so");

    let churchroad_src_path = test_output_dir.join(format!("{}.egg", top_module_name));

    let yosys_commands = format!(
        "read_verilog -sv {}; prep -top {}; pmuxtree; write_lakeroad",
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
    let serialized = egraph.serialize(SerializeConfig::default());
    let (_, is_output_node) = serialized
        .nodes
        .iter()
        .find(|(_, n)| n.op == "IsPort" && n.children[2] == NodeId::from("Output-0"))
        .unwrap();
    let output_id = is_output_node.children.last().unwrap();
    let (_, output_node) = serialized
        .nodes
        .iter()
        .find(|(node_id, _)| **node_id == *output_id)
        .unwrap();

    (serialized.clone(), output_node.clone())
}

#[test]
fn test_lut6_0() {
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

    let include_dirs = vec![churchroad_dir.join("tests/interpreter_tests/verilog/")];

    verilator_vs_interpreter(
        100,
        5,
        testbench_template_path,
        "LUT6",
        inputs,
        outputs,
        include_dirs,
        std::env::temp_dir(),
        churchroad_dir.join("tests/interpreter_tests/verilog/LUT6-modified.v"),
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
            (0..num_clock_cycles + 1)
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
        for timestep in 0..num_clock_cycles + 1 {
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

    let testbench_prog = std::fs::read_to_string(testbench_template_path)
        .unwrap()
        .replace("{filename}", filename)
        // TODO(@ninehusky): this'll eventually need to include parameters as well, right?
        .replace(
            "{input_output_declarations}",
            format!(
                "{}",
                outputs
                    .iter()
                    .map(|(name, bw)| format!("logic [{}:0] {};\n", bw - 1, name))
                    .collect::<Vec<String>>()
                    .join("\n")
            )
            .as_str(),
        )
        .replace("{test_module_name}", top_module_name)
        .replace(
            "{test_module_port_list}",
            format!(
                "{}, {}",
                inputs
                    .iter()
                    .enumerate()
                    .map(|(i, (name, _))| format!(".{}(inputs[{}])", name, i))
                    .collect::<Vec<String>>()
                    .join(",\n"),
                outputs
                    .iter()
                    .map(|(name, _)| format!(".{}({})", name, name))
                    .collect::<Vec<String>>()
                    .join(", ")
                    .as_str()
            )
            .as_str(),
        )
        .replace(
            "{max_input_bitwidth}",
            inputs
                .iter()
                .map(|(_, bitwidth)| bitwidth)
                .max()
                .unwrap()
                .to_string()
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

    let executable_name = "executable";
    let verilator_output_dir = test_output_dir.join("obj_dir");
    let executable_path = verilator_output_dir.join(executable_name);

    std::fs::write(&testbench_path, &testbench_prog).unwrap();

    let verilator_compile_output = std::process::Command::new("verilator")
        .arg("-o")
        .arg(executable_name)
        .arg("-Wno-WIDTHTRUNC")
        .arg("--assert")
        .arg("--timing")
        .arg("--main")
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
        .filter(|line| line.len() > 0 && line.starts_with("output: "))
        .map(|line| line.trim_start_matches("output: ").parse().unwrap())
        .collect();

    fs::write(test_output_dir.join("output.txt"), output_str).unwrap();

    verilator_output_values
}

macro_rules! interpreter_test {
    ($test_name:ident, $expected:expr, $verilog_path:literal, $module_name:literal, $time:literal, $env:expr, $out: literal) => {
        #[test]
        fn $test_name() {
            let (serialized, root_node) = prep_interpreter(
                PathBuf::from($verilog_path),
                std::env::temp_dir(),
                $module_name,
            );

            assert_eq!(
                $expected,
                interpret(&serialized, &root_node.eclass, $time, $env).unwrap()
            );
        }
    };
}

interpreter_test!(
    test_alu_and_single_cycle,
    InterpreterResult::Bitvector(0b01010101, 8),
    "tests/interpreter_tests/verilog/ALU.sv",
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

interpreter_test!(
    test_alu_and_single_cycle_2,
    InterpreterResult::Bitvector(0b01010101, 8),
    "tests/interpreter_tests/verilog/ALU.sv",
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

interpreter_test!(
    test_alu_or_second_cycle,
    InterpreterResult::Bitvector(0b10101010, 8),
    "tests/interpreter_tests/verilog/ALU.sv",
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

    let include_dirs = vec![churchroad_dir.join("tests/interpreter_tests/verilog/")];

    assert_eq!(
        run_verilator(
            testbench_template_path.clone(),
            "LUT6",
            inputs.clone(),
            outputs.clone(),
            vec![vec![vec![0, 0, 0, 0, 0, 0, 0]]],
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
