// This file contains tests for the interpreter module.

use std::{collections::HashMap, io::Write, path::PathBuf};

use rand::{Rng, RngCore};

use egglog::{EGraph, SerializeConfig};

use churchroad::{import_churchroad, interpret, InterpreterResult};

macro_rules! interpreter_test {
    ($test_name:ident, $expected:expr, $filename:literal, $time:literal, $env:expr, $out: literal) => {
        #[test]
        fn $test_name() {
            let program = std::fs::read_to_string($filename).unwrap();
            let mut egraph: EGraph = EGraph::default();
            import_churchroad(&mut egraph);
            egraph.parse_and_run_program(&program).unwrap();
            egraph
                .parse_and_run_program(
                    format!("(relation IsRoot (Expr)) (IsRoot {})", $out).as_str(),
                )
                .unwrap();
            let serialized = egraph.serialize(SerializeConfig::default());
            let (_, is_root_node) = serialized
                .nodes
                .iter()
                .find(|(_, n)| n.op == "IsRoot")
                .unwrap();
            if is_root_node.children.len() != 1 {
                panic!("IsRoot relation must have exactly one child");
            }
            let root_id = is_root_node.children.first().unwrap();
            let (_, root_node) = serialized
                .nodes
                .iter()
                .find(|(node_id, _)| **node_id == *root_id)
                .unwrap();

            assert_eq!(
                $expected,
                interpret(&serialized, &root_node.eclass, $time, $env).unwrap()
            );
        }
    };
}

fn prep_interpreter(churchroad_prog_path: PathBuf, output_name: &str) -> (egraph_serialize::EGraph, egraph_serialize::Node) {
    // prep egraph for interpretation
    let mut egraph: EGraph = EGraph::default();
    import_churchroad(&mut egraph);
    egraph
        .parse_and_run_program(
            &std::fs::read_to_string(churchroad_prog_path).unwrap(),
        )
        .unwrap();
    egraph
        .parse_and_run_program(format!("(relation IsRoot (Expr)) (IsRoot {})", output_name).as_str())
        .unwrap();
    let serialized = egraph.serialize(SerializeConfig::default());
    let (_, is_root_node) = serialized
        .nodes
        .iter()
        .find(|(_, n)| n.op == "IsRoot")
        .unwrap();
    if is_root_node.children.len() != 1 {
        panic!("IsRoot relation must have exactly one child");
    }
    let root_id = is_root_node.children.first().unwrap();
    let (_, root_node) = serialized
        .nodes
        .iter()
        .find(|(node_id, _)| **node_id == *root_id)
        .unwrap();

    (serialized.clone(), root_node.clone())
}

#[test]
fn test_with_verilator() {
    if std::env::var("CHURCHROAD_DIR").is_err() {
        panic!("Please set the CHURCHROAD_DIR environment variable!");
    }
    let churchroad_dir_str: String = std::env::var("CHURCHROAD_DIR").unwrap();
    let churchroad_dir = std::path::Path::new(&churchroad_dir_str);
    let testbench_template_path =
        churchroad_dir.join("tests/interpreter_tests/verilog/testbench.sv.template");
    let makefile_template_path = churchroad_dir.join("tests/interpreter_tests/Makefile.template");

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

    let include_dirs = vec![
        churchroad_dir.join("tests/interpreter_tests/verilog/actual"),
        churchroad_dir.join("tests/interpreter_tests/verilog/"),
    ];

    verilator(
        testbench_template_path,
        makefile_template_path,
        "LUT6",
        inputs,
        outputs,
        include_dirs,
        std::env::temp_dir(),
        churchroad_dir.join("tests/interpreter_tests/LUT6-modified.egg"),
        10,
        1
    );
}

// This test runs verilator against our interpreter.
//
//
// test_module_name: str,
// ground_truth_module_name: str,
// obj_dirpath: Union[str, Path],
// verilog_filepaths: List[Union[str, Path]],
// module_inputs: List[Tuple[str, int]],
// initiation_interval: int,
// testbench_sv_filepath: Union[str, Path],
// testbench_exe_filepath: Union[str, Path],
// testbench_inputs_filepath: Union[str, Path],
// testbench_stdout_log_filepath: Union[str, Path],
// testbench_stderr_log_filepath: Union[str, Path],
// makefile_filepath: Union[str, Path],
// module_outputs: List[Tuple[str, int]],
// clock_name: Optional[str] = None,
// include_dirs: List[Union[str, Path]] = [],
// extra_args: List[str] = [],
// max_num_tests=MAX_NUM_TESTS,
// ignore_missing_test_module_file: bool = False,
// expect_all_zero_outputs: bool = False,
fn verilator(
    testbench_template_path: PathBuf,
    makefile_template_path: PathBuf,
    top_module_name: &str,
    inputs: Vec<(&str, i32)>,
    outputs: Vec<(&str, i32)>,
    include_dirs: Vec<PathBuf>,
    test_output_dir: PathBuf,
    churchroad_src_path: PathBuf,
    num_test_cases: usize,
    num_clock_cycles: usize,
) {
    if std::env::var("CXX").is_err() {
        std::env::set_var("CXX", "clang++ -std=c++20")
    }
    if std::env::var("BETTER_VERILATOR").is_err() {
        std::env::set_var(
            "BETTER_VERILATOR",
            "/Users/andrew/research/verilator/bin/verilator",
        )
    }

    let testbench_path = test_output_dir.join("testbench.sv");
    let makefile_path = test_output_dir.join("Makefile");

    let testbench_prog = std::fs::read_to_string(testbench_template_path)
        .unwrap()
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
                format!(
                    "{}",
                    inputs
                        .iter()
                        .enumerate()
                        .map(|(i, (name, _))| format!(".{}(inputs[{}])\n", name, i))
                        .collect::<Vec<String>>()
                        .join(", ")
                ),
                format!(
                    "{}",
                    outputs
                        .iter()
                        .map(|(name, _)| format!(".{}({})", name, name))
                        .collect::<Vec<String>>()
                        .join(", ")
                        .as_str()
                )
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
        );

    let executable_name = "executable";
    let verilator_output_dir = test_output_dir.join("obj_dir");
    let executable_path = verilator_output_dir.join(executable_name);

    // TODO(@ninehusky): test mii
    let default_extra_args = format!(
        "-I{}",
        include_dirs
            .iter()
            .map(|path| path.to_str().unwrap())
            .collect::<Vec<&str>>()
            .join(" -I")
    );

    let makefile_prog = std::fs::read_to_string(makefile_template_path)
        .unwrap()
        .replace("{testbench_file_path}", testbench_path.to_str().unwrap())
        .replace(
            "{verilator_output_dir}",
            verilator_output_dir.to_str().unwrap(),
        )
        .replace("{simulation_executable_name}", executable_name)
        .replace("{extra_verilator_args}", &default_extra_args);

    std::fs::write(&testbench_path, &testbench_prog).unwrap();
    std::fs::write(&makefile_path, &makefile_prog).unwrap();

    // TODO(@ninehusky): We can get rid of the necessity for a Makefile after this PR is merged
    // into Verilator: https://github.com/verilator/verilator/pull/5031
    let verilator_compile_output = std::process::Command::new("make")
        .arg("--environment-overrides")
        .arg("--always-make")
        .arg("-f")
        .arg(makefile_path)
        .output()
        .unwrap();

    if !verilator_compile_output.status.success() {
        panic!(
            "Verilator failed, stderr: {:?}",
            String::from_utf8(verilator_compile_output.stderr)
        );
    }

    // simulation process
    let mut sim_proc = std::process::Command::new(executable_path)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();

    let sim_proc_stdin = sim_proc.stdin.as_mut().unwrap();


    // i'll clean this up later
    let num_inputs = inputs.len();

    sim_proc_stdin
        .write_all(format!("{} {} {}\n", num_inputs, num_test_cases, num_clock_cycles).as_bytes())
        .unwrap();

    let mut rng = rand::thread_rng();
    let mut interpreter_results: Vec<InterpreterResult> = Vec::new();
    for _ in 0..num_test_cases {
        for t in 0..num_clock_cycles {
            let mut env: HashMap<&str, Vec<u64>> = HashMap::new();
            let input_values: Vec<u64> = inputs
                .iter()
                .map(|(name, bw)| {
                    // generate value in [0, 2^bw - 1]
                    let mut val = rng.next_u64();
                    if *bw != 64 {
                        val = val & ((1 << bw) - 1);
                    }
                    env.insert(name, vec![val]);
                    val
                })
                .collect();
            
            for input in input_values.iter() {
                sim_proc_stdin
                    .write_all(format!("{:X}\n", input).as_bytes())
                    .unwrap();
            }
            
            // TODO(@ninehusky): this is going to assume we only want to interpret on the first output.
            let (serialized, root_node) = prep_interpreter(churchroad_src_path.clone(), outputs.first().unwrap().0);

            let result = interpret(&serialized, &root_node.eclass, t, &env).unwrap();
            interpreter_results.push(result);
        }
    }

    let output = sim_proc.wait_with_output().unwrap();

    let output_str = String::from_utf8(output.stdout).unwrap();
    let verilator_output_values: Vec<u64> = output_str
        .lines()
        // filter all lines that don't start with "output: "
        .filter(|line| line.len() > 0 && line.starts_with("output: "))
        .map(|line| line.trim_start_matches("output: ").parse().unwrap())
        .collect();

    for (interpreter_result, verilator_result) in interpreter_results.iter().zip(verilator_output_values.iter()) {
        match interpreter_result {
            InterpreterResult::Bitvector(val, _) => {
                // TODO(@ninehusky): there is potentially suspicious unit conversion stuff happening here? hopefully is simple cast
                assert_eq!(*val, *verilator_result);
            }
            _ => panic!("expected bitvector result"),
        }
    }

    let test_output_path = test_output_dir.join("test_output.txt");
    let test_error_path = test_output_dir.join("test_error.txt");

    std::fs::write(&test_output_path, output_str).unwrap();
    std::fs::write(&test_error_path, output.stderr).unwrap();

    println!("logged output to: {}", test_output_path.to_str().unwrap());
}

interpreter_test!(
    test_alu_0,
    InterpreterResult::Bitvector(0b01010101, 8),
    "tests/interpreter_tests/ALU.egg",
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
    test_alu_1,
    InterpreterResult::Bitvector(0b11111111, 8),
    "tests/interpreter_tests/ALU.egg",
    0,
    &[
        ("a", vec![0b01010101]),
        ("b", vec![0b11111111]),
        ("op", vec![0])
    ]
    .into(),
    "out"
);

// TODO(@ninehusky): Fix this test, it's currently broken until we fix the interpreter
// and use the non-broken LUT6.egg file.
interpreter_test!(
    test_lut6_0,
    // grab the only 1-bit from INIT
    InterpreterResult::Bitvector(0b1, 1),
    "tests/interpreter_tests/LUT6-modified.egg",
    0,
    &[
        ("INIT", vec![0x0000000000000002]),
        ("I0", vec![0b1]),
        ("I1", vec![0b0]),
        ("I2", vec![0b0]),
        ("I3", vec![0b0]),
        ("I4", vec![0b0]),
        ("I5", vec![0b0]),
    ]
    .into(),
    "O"
);

// TODO(@ninehusky): Fix this test, it's currently broken until we fix the interpreter
// and use the non-broken LUT6.egg file.
interpreter_test!(
    test_lut6_1,
    // grab the only 1-bit from INIT
    InterpreterResult::Bitvector(0b1, 1),
    "tests/interpreter_tests/LUT6-modified.egg",
    0,
    &[
        ("INIT", vec![0x0000000000000010]),
        ("I0", vec![0b0]),
        ("I1", vec![0b0]),
        ("I2", vec![0b1]),
        ("I3", vec![0b0]),
        ("I4", vec![0b0]),
        ("I5", vec![0b0]),
    ]
    .into(),
    "O"
);

// TODO(@ninehusky): Fix this test, it's currently broken until we fix the interpreter
// and use the non-broken LUT6.egg file.
interpreter_test!(
    test_lut6_2,
    InterpreterResult::Bitvector(0b1, 1),
    "tests/interpreter_tests/LUT6-modified.egg",
    0,
    &[
        ("INIT", vec![398976]),
        ("I0", vec![0b0]),
        ("I1", vec![0b1]),
        ("I2", vec![0b0]),
        ("I3", vec![0b1]),
        ("I4", vec![0b0]),
        ("I5", vec![0b0]),
    ]
    .into(),
    "O"
);