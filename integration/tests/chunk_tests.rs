use integration::{
    prove::{prove_and_verify_chunk, prove_and_verify_sp1_chunk, SP1Prover},
    test_util::{load_chunk, trace_path_for_test, ASSETS_DIR, PARAMS_DIR},
};
use prover::{init_env_and_log, ChunkProvingTask, CHUNK_PROVER_DEGREES};

#[cfg(feature = "prove_verify")]
#[test]
fn test_chunk_prove_verify() {
    use itertools::Itertools;
    let output_dir = init_env_and_log("chunk_tests");
    log::info!("Initialized ENV and created output-dir {output_dir}");

    let params_map = prover::Prover::load_params_map(
        PARAMS_DIR,
        &CHUNK_PROVER_DEGREES.iter().copied().collect_vec(),
    );

    let trace_path = trace_path_for_test();
    let traces = load_chunk(&trace_path).1;
    let chunk = ChunkProvingTask::new(traces);
    let mut prover = prover::ChunkProver::from_params_and_assets(&params_map, ASSETS_DIR);
    log::info!("Constructed chunk prover");
    prove_and_verify_chunk(&params_map, &output_dir, chunk, &mut prover, None, true);
}

#[cfg(feature = "prove_verify")]
#[test]
fn test_sp1_chunk_prove_verify() {
    use itertools::Itertools;
    use std::path::Path;
    let output_dir = init_env_and_log("chunk_tests");
    log::info!("Initialized ENV and created output-dir {output_dir}");

    let params_map = prover::Prover::load_params_map(
        PARAMS_DIR,
        &CHUNK_PROVER_DEGREES.iter().copied().collect_vec(),
    );

    let trace_path = trace_path_for_test();

    // suppose the snark is put in the same path with trace_path
    // and we extract the path part
    let trace_asset_path = Path::new(&trace_path);
    let trace_asset_path = if trace_asset_path.is_dir() {
        trace_asset_path.to_path_buf()
    } else {
        trace_asset_path
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| Path::new(".").to_path_buf())
    };

    let traces = load_chunk(&trace_path).1;
    let chunk = ChunkProvingTask::new(traces);
    let mut prover = SP1Prover::from_params_and_assets(&params_map, ASSETS_DIR);
    log::info!("Constructed sp1 chunk prover");
    prove_and_verify_sp1_chunk(
        &params_map,
        &output_dir,
        Some(trace_asset_path.to_str().unwrap()),
        chunk,
        &mut prover,
        None,
    );
}
