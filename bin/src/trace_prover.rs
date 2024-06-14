use clap::Parser;
use integration::{prove::prove_and_verify_chunk, test_util::load_chunk};
use prover::{utils::init_env_and_log, ChunkProvingTask};
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
        default_value = "tests/extra_traces/batch_34700/chunk_1236462/block_4176564.json"
    )]
    trace_path: String,
}

fn main() {
    // Layer config files are located in `./integration/configs`.
    env::set_current_dir("./integration").unwrap();
    let output_dir = init_env_and_log("trace_prover");
    log::info!("Initialized ENV and created output-dir {output_dir}");

    let args = Args::parse();

    let traces = load_chunk(&args.trace_path).1;
    prover::eth_types::constants::set_scroll_block_constants_with_trace(&traces[0]);
    let chunk = ChunkProvingTask::from(traces);
    prove_and_verify_chunk(
        chunk,
        Some("0"), // same with `make test-chunk-prove`, to load vk
        &args.params_path,
        &args.assets_path,
        &output_dir,
    );
    log::info!("chunk prove done");
}
