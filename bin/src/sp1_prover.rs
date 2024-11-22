use clap::Parser;
use integration::{prove::prove_and_verify_sp1_chunk, test_util::load_chunk_compatible};
use scroll_prover::{init_env_and_log, Sp1Prover, ChunkProvingTask};
use std::env;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Get params dir path.
    #[clap(short, long = "params", default_value = "params")]
    params_path: String,
    /// Get asserts dir path.
    #[clap(short, long = "assets", default_value = "test_assets")]
    assets_path: String,
    /// Get BlockTrace from file or dir.
    #[clap(
        short,
        long = "trace",
        default_value = "tests/extra_traces/batch_sproll/5224657.json"
    )]
    trace_path: String,
}

const DEFAULT_PARAM : &str = 
r#"{
  "strategy": "Vertical",
  "k": 24,
  "num_advice_per_phase": [
    3
  ],
  "num_fixed": 1,
  "num_lookup_advice_per_phase": [
    1
  ],
  "lookup_bits": 22
}"#;

fn main() {
    // Layer config files are located in `./integration/configs`.
    env::set_current_dir("./integration").unwrap();
    let output_dir = init_env_and_log("sp1_prover");
    log::info!("Initialized ENV and created output-dir {output_dir}");

    let args = Args::parse();

    let traces = load_chunk_compatible(&args.trace_path).1;
    prover::eth_types::constants::set_scroll_block_constants_with_trace(&traces[0]);
    let chunk = ChunkProvingTask::new(traces);
    let params_map = prover::Prover::load_params_map(
        &args.params_path,
        &[
            *prover::INNER_DEGREE,
            *prover::LAYER1_DEGREE,
            *prover::LAYER2_DEGREE,
        ],
    );
    log::warn!("must set VERIFY_VK to false, we also enfore SHARD_SIZE");
    env::set_var("VERIFY_VK", "false");
    env::set_var("SHARD_SIZE", "524288");
    env::set_var("BASE_CONFIG_PARAMS", DEFAULT_PARAM);
    let mut prover = Sp1Prover::from_params_and_assets(&params_map, &args.assets_path);
    log::info!("Constructed sp1 chunk prover");

    prove_and_verify_sp1_chunk(
        &params_map,
        &output_dir,
        chunk,
        &mut prover,
        Some("0"), // same with `make test-chunk-prove`, to load vk        
    );
    log::info!("sp1 prove done");
}
