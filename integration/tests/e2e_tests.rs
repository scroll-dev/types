use integration::test_util::{
    gen_and_verify_batch_proofs, load_chunk, load_chunk_for_test, BatchProvingTask, ASSETS_DIR,
    PARAMS_DIR,
};
use prover::{
    aggregator::Prover,
    utils::{chunk_trace_to_witness_block, init_env_and_log, read_env_var},
    zkevm, BatchHash, ChunkHash, ChunkProvingTask,
};
use std::env;

fn load_test_batch() -> anyhow::Result<Vec<String>> {
    let batch_dir = read_env_var("TRACE_PATH", "./tests/extra_traces/batch_25".to_string());
    load_batch(&batch_dir)
}

fn load_batch(batch_dir: &str) -> anyhow::Result<Vec<String>> {
    let mut sorted_dirs: Vec<String> = std::fs::read_dir(batch_dir)?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .map(|path| path.to_string_lossy().into_owned())
        .collect::<Vec<String>>();
    sorted_dirs.sort_by_key(|s| {
        // Remove the "chunk_" prefix and parse the remaining part as an integer
        s.trim_start_matches("chunk_").parse::<u32>().unwrap()
    });
    let fast = false;
    if fast {
        sorted_dirs.truncate(1);
    }
    log::info!("batch content: {:?}", sorted_dirs);
    Ok(sorted_dirs)
}

#[test]
fn test_batch_pi_consistency() {
    let output_dir = init_env_and_log("batch_pi");
    log::info!("Initialized ENV and created output-dir {output_dir}");
    let trace_paths = load_test_batch().unwrap();
    log_batch_pi(&trace_paths);
}

#[cfg(feature = "prove_verify")]
#[test]
fn test_e2e_prove_verify() {
    let output_dir = init_env_and_log("agg_tests");
    log::info!("Initialized ENV and created output-dir {output_dir}");

    let chunk_dirs = load_batch("./tests/extra_traces/batch_5").unwrap();
    let batch = gen_chunk_hashes_and_proofs(&output_dir, &chunk_dirs);

    let mut batch_prover = new_batch_prover(&output_dir);
    prove_and_verify_batch(&output_dir, &mut batch_prover, batch);
}

fn gen_chunk_hashes_and_proofs(output_dir: &str, chunk_dirs: &[String]) -> BatchProvingTask {
    let chunks: Vec<_> = chunk_dirs
        .iter()
        .map(|chunk_dir| load_chunk(chunk_dir).1)
        .collect();

    let mut zkevm_prover = zkevm::Prover::from_dirs(PARAMS_DIR, ASSETS_DIR);
    log::info!("Constructed zkevm prover");
    let chunk_proofs: Vec<_> = chunks
        .into_iter()
        .enumerate()
        .map(|(i, block_traces)| {
            zkevm_prover
                .gen_chunk_proof(
                    ChunkProvingTask { block_traces },
                    Some(&i.to_string()),
                    None,
                    Some(output_dir),
                )
                .unwrap()
        })
        .collect();

    log::info!("Generated chunk proofs");
    BatchProvingTask { chunk_proofs }
}

fn log_batch_pi(trace_paths: &[String]) {
    let max_num_snarks = prover::MAX_AGG_SNARKS;
    let chunk_traces: Vec<_> = trace_paths
        .iter()
        .map(|trace_path| {
            env::set_var("TRACE_PATH", trace_path);
            load_chunk_for_test().1
        })
        .collect();

    let mut chunk_hashes: Vec<ChunkHash> = chunk_traces
        .into_iter()
        .enumerate()
        .map(|(_i, chunk_trace)| {
            let witness_block = chunk_trace_to_witness_block(chunk_trace.clone()).unwrap();
            ChunkHash::from_witness_block(&witness_block, false)
        })
        .collect();

    let real_chunk_count = chunk_hashes.len();
    if real_chunk_count < max_num_snarks {
        let mut padding_chunk_hash = chunk_hashes.last().unwrap().clone();
        padding_chunk_hash.is_padding = true;

        // Extend to MAX_AGG_SNARKS for both chunk hashes and layer-2 snarks.
        chunk_hashes
            .extend(std::iter::repeat(padding_chunk_hash).take(max_num_snarks - real_chunk_count));
    }

    let batch_hash = BatchHash::<{ prover::MAX_AGG_SNARKS }>::construct(&chunk_hashes);
    let blob = batch_hash.point_evaluation_assignments();

    let challenge = blob.challenge;
    let evaluation = blob.evaluation;
    println!("blob.challenge: {challenge:x}");
    println!("blob.evaluation: {evaluation:x}");
    for (i, elem) in blob.coefficients.iter().enumerate() {
        println!("blob.coeffs[{}]: {elem:x}", i);
    }
}

fn new_batch_prover(assets_dir: &str) -> Prover {
    env::set_var("AGG_VK_FILENAME", "vk_batch_agg.vkey");
    env::set_var("CHUNK_PROTOCOL_FILENAME", "chunk_chunk_0.protocol");
    let prover = Prover::from_dirs(PARAMS_DIR, assets_dir);
    log::info!("Constructed batch prover");

    prover
}

fn prove_and_verify_batch(output_dir: &str, batch_prover: &mut Prover, batch: BatchProvingTask) {
    // Load or generate aggregation snark (layer-3).
    let chunk_hashes_proofs: Vec<_> = batch.chunk_proofs[..]
        .iter()
        .cloned()
        .map(|p| (p.chunk_hash.clone().unwrap(), p))
        .collect();
    let layer3_snark = batch_prover
        .load_or_gen_last_agg_snark("agg", chunk_hashes_proofs, Some(output_dir))
        .unwrap();

    gen_and_verify_batch_proofs(batch_prover, layer3_snark, output_dir);
}
