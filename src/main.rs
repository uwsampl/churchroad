use std::collections::HashMap;
use std::fs::create_dir_all;
use std::path::PathBuf;

use churchroad::{
    call_lakeroad_on_primitive_interface_and_spec, find_primitive_interfaces_serialized,
    find_spec_for_primitive_interface, from_verilog_file, node_to_string,
    to_verilog_egraph_serialize, StructuralVerilogExtractor,
};
use clap::ValueHint::FilePath;
use clap::{ArgAction, Parser, ValueEnum};
use egglog::SerializeConfig;
use log::{debug, info, warn};
use tempfile::NamedTempFile;

/// Simple program to greet a person
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(long, value_hint=FilePath)]
    filepath: PathBuf,

    #[arg(long)]
    top_module_name: String,

    #[arg(long)]
    svg_dirpath: Option<PathBuf>,

    #[arg(long)]
    architecture: Architecture,

    #[arg(long)]
    simulate: bool,

    #[arg(long)]
    out_filepath: Option<PathBuf>,

    #[arg(long, action=ArgAction::Append)]
    simulate_with_verilator_arg: Vec<String>,
}

#[derive(ValueEnum, Clone, Debug)]
enum Architecture {
    XilinxUltrascalePlus,
}

// TODO(@gussmith23): Seems redundant to do this; I think clap already does something like this under the hood.
impl ToString for Architecture {
    fn to_string(&self) -> String {
        match self {
            Architecture::XilinxUltrascalePlus => "xilinx-ultrascale-plus".to_owned(),
        }
    }
}

fn main() {
    env_logger::init();
    let args = Args::parse();

    // STEP 1: Read in design, put it in an egraph.
    // simcheck=true just runs some basic checks.
    let mut egraph = from_verilog_file(
        &args.filepath,
        &args.top_module_name,
        true,
        true,
        HashMap::default(),
    );

    info!("Loaded design into egraph.");

    if let Some(svg_dirpath) = &args.svg_dirpath {
        create_dir_all(svg_dirpath).unwrap();
        let serialized = egraph.serialize_for_graphviz(true, usize::MAX, usize::MAX);
        serialized
            .to_svg_file(svg_dirpath.join("initial_egraph.svg"))
            .unwrap();
        info!(
            "Initial egraph svg: {}",
            svg_dirpath.join("initial_egraph.svg").to_string_lossy()
        );
    }

    // STEP 2: Run mapping rewrites, proposing potential mappings which Lakeroad
    // will later confirm or prove not possible via program synthesis.
    //
    // Currently, we only have a single rewrite, which looks for "narrower"
    // multiplies which should fit on a single DSP.
    //
    // In the future, there's much more we can do here, including:
    // - Parameterizing rewrites based on architecture (i.e. instead of
    //   hardcoding "18" below, we can get the appropriate width from the
    //   architecture description.)
    // - Mixing mapping rewrites with "expansion" rewrites. For example, adding
    //   a rewrite which breaks a large multiply into smaller multiplies. Again,
    //   these rewrites can also be parameterized by arch. descr.
    // - Automated generation of rewrites. This is a more interesting research
    //   question! Could be a place we use ChatGPT; i.e. give it the PDF of
    //   the DSP manual, give it a description of the Churchroad IR, and ask it
    //   to propose patterns.
    info!("Running rewrites.");
    egraph
        .parse_and_run_program(
            r#"
        (ruleset mapping)
        (rule 
            ((= expr (Op2 (Mul) a b))
             (HasType expr (Bitvector n))
             (< n 18))
            ((union expr (PrimitiveInterfaceDSP a b)))
            :ruleset mapping)
        ;; TODO need to write a rewrite that deals with multiplying zero extended bvs
        (rule 
            ((= expr (Op2 (Mul) a b))
             (HasType expr (Bitvector n))
             (< n 18))
            ((union expr (PrimitiveInterfaceDSP a b)))
            :ruleset mapping)
        
        (ruleset transform)
        (rule
            ((= expr (Op2 (Mul) (Op1 (ZeroExtend b-bw) a) b))
             (HasType expr (Bitvector expr-bw))
             (HasType a (Bitvector a-bw))
             (HasType b (Bitvector b-bw))
             (<= expr-bw 48)
             (<= a-bw 16)
             (<= b-bw 32)
             (= 0 (% expr-bw 2)))
            ((union 
               expr 
               (Op2 (Add)
                (Op2 (Mul) (Op1 (ZeroExtend expr-bw) a) (Op1 (ZeroExtend expr-bw) (Op1 (Extract (- (/ expr-bw 2) 1) 0) b)))
                (Op2 (Shl) (Op2 (Mul) (Op1 (ZeroExtend expr-bw) a) (Op1 (ZeroExtend expr-bw) (Op1 (Extract (- expr-bw 1) (/ expr-bw 2)) b))) (Op0 (BV (/ expr-bw 2) expr-bw))))))
            :ruleset transform)
    "#,
        )
        .unwrap();
    egraph
        .parse_and_run_program("(run-schedule (saturate typing))")
        .unwrap();
    egraph
        .parse_and_run_program("(run-schedule (saturate transform))")
        .unwrap();
    egraph
        .parse_and_run_program("(run-schedule (saturate mapping))")
        .unwrap();

    // May need this rebuild. See
    // https://github.com/egraphs-good/egglog/pull/391
    // egraph.rebuild();

    if let Some(svg_dirpath) = &args.svg_dirpath {
        create_dir_all(svg_dirpath).unwrap();
        let serialized = egraph.serialize_for_graphviz(true, usize::MAX, usize::MAX);
        serialized
            .to_svg_file(svg_dirpath.join("after_rewrites.svg"))
            .unwrap();
        info!(
            "Egraph after rewrites: {}",
            svg_dirpath.join("after_rewrites.svg").to_string_lossy()
        );
    }

    let serialized_egraph = egraph.serialize(SerializeConfig::default());

    // STEP 3: Collect all proposed mappings.
    // In this step, we simply find all mapping proposals, i.e. all places where
    // the above rewrites *think* we might be able to use a DSP. In the next
    // step, we'll actually confirm or deny whether these mappings can work.
    //
    // In the future, this step might also involve ranking potential mapping
    // proposals, because in a large design, there will likely be many of them!
    // There are many potential ways to rank: heuristics, cost models, etc.
    //
    //
    // TODO(@gussmith23): Make this return Vec<(choices, nodeid)>.
    // Basically it can have the same API as the spec finding function. They're
    // both doing very similar things: basically, an extraction. They're just
    // extracting different things for the same classes.
    let node_ids = find_primitive_interfaces_serialized(&serialized_egraph);

    info!(
        "Found {} potential mappings; running Lakeroad on each.",
        node_ids.len()
    );

    // STEP 5: For each proposed mapping, attempt synthesis with Lakeroad.
    for sketch_template_node_id in &node_ids {
        // TODO(@gussmith23): This is a hack, see https://github.com/egraphs-good/egglog/issues/392
        // Doing everything over the serialized egraph, so I don't actually need this anymore.
        // let canonical: usize = egraph.find(*value).bits.try_into().unwrap();
        // let canonical_id: egraph_serialize::ClassId = canonical.to_string().into();
        // let (choices, spec_node_id) =
        //     find_spec_for_primitive_interface(&canonical_id, &serialized_egraph);

        // STEP 5.1: For each proposed mapping, extract a "spec".
        // In the above step, we extracted all of the proposed mapping nodes.
        // These nodes are just markers that say "this eclass could potentially
        // be implemented with a DSP." To actually do synthesis with Lakeroad,
        // we need to extract *yet another* representative from the eclass:
        // one that can serve as a specification which Lakeroad can synthesize
        // against. Currently, this mostly just means extracting *any*
        // expression from the eclass which can be converted to valid Verilog.
        //
        // In the future, we could also consider extracting *multiple*
        // representatives per eclass, which gives us more specs to attempt
        // synthesis against. Given that solvers are strange and often benefit
        // from running in a portfolio, having many equivalent specs might
        // increase chances at synthesis termination.
        let (spec_choices, spec_node_id) = find_spec_for_primitive_interface(
            &serialized_egraph[sketch_template_node_id].eclass,
            &serialized_egraph,
        );

        log::info!(
            "Calling Lakeroad with spec:\n{}\nand sketch:\n{}",
            node_to_string(&serialized_egraph, &spec_node_id, &spec_choices),
            serialized_egraph[sketch_template_node_id].op
        );

        // STEP 5.2: Call Lakeroad.
        let commands = call_lakeroad_on_primitive_interface_and_spec(
            &serialized_egraph,
            &spec_choices,
            &spec_node_id,
            sketch_template_node_id,
            &args.architecture.to_string(),
        );

        log::debug!(
            "First few lines of commands generated from Lakeroad output:\n{}",
            commands.lines().take(10).collect::<String>()
        );

        // STEP 5.3: Insert Lakeroad's results back into the egraph.
        // If Lakeroad finds a mapping, insert the mapping into the egraph.
        // If Lakeroad proves UNSAT, put some kind of marker into the egraph
        // to indicate that this mapping shouldn't be attempted again.
        egraph.parse_and_run_program(&commands).unwrap();

        info!("Inserted Lakeroad's results back into egraph.");
    }

    // Write out image if the user requested it.
    if let Some(svg_dirpath) = args.svg_dirpath {
        let serialized = egraph.serialize_for_graphviz(true, usize::MAX, usize::MAX);
        serialized
            .to_svg_file(svg_dirpath.join("after_lakeroad.svg"))
            .unwrap();
        info!(
            "Egraph after all calls to Lakeroad: {}",
            svg_dirpath.join("after_lakeroad.svg").to_string_lossy()
        );
    }

    // STEP 6: Extract a lowered design.
    //
    // Once we have attempted all mappings, we should ideally be able to extract
    // a design in structural Verilog.
    //
    // Future work at this stage will involve building an extractor which
    // which actually attempts to find an *optimal* design, not just *any*
    // design.

    let serialized = egraph.serialize(SerializeConfig::default());
    let choices = StructuralVerilogExtractor.extract(&serialized, &[]);
    let verilog = to_verilog_egraph_serialize(&serialized, &choices, "clk");

    debug!("Final extracted Verilog:\n{}", &verilog);

    if let Some(out_filepath) = &args.out_filepath {
        std::fs::write(out_filepath, &verilog).unwrap();
    } else {
        println!("{}", verilog);
    }

    // STEP 7: Simulate.
    if args.simulate {
        // If we didn't write to file, we need to write to a temp file.
        let old_verilog_filepath = if let Some(out_filepath) = &args.out_filepath {
            out_filepath.to_owned()
        } else {
            let (_, path) = NamedTempFile::new().unwrap().keep().unwrap();
            std::fs::write(&path, &verilog).unwrap();
            path
        };

        // First, we have to rename the output module, because our
        // simulate_with_verilator.py script can't simulate two modules with the
        // same name against each other.
        let verilog_file = NamedTempFile::new().unwrap();
        // Use Yosys to rename the module.
        let new_module_name = format!("{}_simulate_with_verilator", args.top_module_name);
        let yosys_output = std::process::Command::new("yosys")
            .arg("-p")
            .arg(format!(
                "read_verilog -sv {}; rename {} {}; write_verilog {}",
                old_verilog_filepath.to_string_lossy(),
                //args.top_module_name,
                {
                    warn!("TODO(@gussmith23): hardcoded.");
                    "top"
                },
                new_module_name,
                verilog_file.path().to_string_lossy()
            ))
            .output()
            .unwrap();
        if !yosys_output.status.success() {
            panic!(
                "Yosys failed to rename the module. stdout:\n{}\nstderr:\n{}",
                String::from_utf8_lossy(&yosys_output.stdout),
                String::from_utf8_lossy(&yosys_output.stderr)
            );
        }

        let lakeroad_dir = PathBuf::from(
            std::env::var("LAKEROAD_DIR")
                .expect("LAKEROAD_DIR environment variable should be set."),
        );

        let mut cmd = std::process::Command::new("python");
        cmd.arg(lakeroad_dir.join("bin").join("simulate_with_verilator.py"))
            .arg("--verilog_filepath")
            .arg(verilog_file.path())
            .arg("--test_module_name")
            .arg(new_module_name)
            .arg("--ground_truth_module_name")
            .arg(args.top_module_name)
            // TODO(@gussmith23): hardcoded.
            .arg("--output_signal")
            .arg({
                warn!("hardcoded");
                "out:16"
            })
            .arg("--input_signal")
            .arg({
                warn!("hardcoded");
                "a:16"
            })
            .arg("--input_signal")
            .arg({
                warn!("hardcoded");
                "b:16"
            })
            .arg("--verilator_extra_arg")
            .arg(args.filepath)
            .args(args.simulate_with_verilator_arg);

        let output = cmd.output().unwrap();

        if !output.status.success() {
            panic!(
                "Simulation with Verilator failed. stdout:\n{}\nstderr:\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }

        info!("Simulation with Verilator succeeded.");
    }
}
