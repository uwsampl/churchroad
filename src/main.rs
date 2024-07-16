use std::path::PathBuf;

use churchroad::{
    call_lakeroad_on_primitive_interface_and_spec, find_primitive_interface_values,
    find_primitive_interfaces_serialized, find_spec_for_primitive_interface, from_verilog_file,
};
use clap::ValueHint::FilePath;
use clap::{Parser, ValueEnum};
use egglog::SerializeConfig;
use egraph_serialize::NodeId;

/// Simple program to greet a person
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(long, value_hint=FilePath)]
    filepath: PathBuf,

    #[arg(long)]
    top_module_name: String,

    #[arg(long)]
    svg_filepath: Option<PathBuf>,

    #[arg(long)]
    architecture: Architecture,
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
    let args = Args::parse();

    // STEP 1: Read in design, put it in an egraph.
    let mut egraph = from_verilog_file(&args.filepath, &args.top_module_name);

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
    "#,
        )
        .unwrap();
    egraph
        .parse_and_run_program("(run-schedule (saturate typing))")
        .unwrap();
    egraph
        .parse_and_run_program("(run-schedule (saturate mapping))")
        .unwrap();

    // May need this rebuild. See
    // https://github.com/egraphs-good/egglog/pull/391
    // egraph.rebuild();

    // Write out image if the user requested it.
    if let Some(svg_filepath) = args.svg_filepath {
        let serialized = egraph.serialize_for_graphviz(true, usize::MAX, usize::MAX);
        serialized.to_svg_file(svg_filepath).unwrap();
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

        // STEP 5.2: Call Lakeroad.
        call_lakeroad_on_primitive_interface_and_spec(
            &serialized_egraph,
            &spec_choices,
            &spec_node_id,
            &sketch_template_node_id,
            &args.architecture.to_string(),
        );

        // STEP 5.3: Insert Lakeroad's results back into the egraph.
        // If Lakeroad finds a mapping, insert the mapping into the egraph.
        // If Lakeroad proves UNSAT, put some kind of marker into the egraph
        // to indicate that this mapping shouldn't be attempted again.
    }
    
    // STEP 6: Extract a lowered design.
    //
    // Once we have attempted all mappings, we should ideally be able to extract
    // a design in structural Verilog.
    //
    // Future work at this stage will involve building an extractor which
    // which actually attempts to find an *optimal* design, not just *any*
    // design.
}