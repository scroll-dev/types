use super::PARAMS_DIR;
use prover::{
    aggregator::{Prover as BatchProver, Verifier as BatchVerifier},
    zkevm::{Prover as ChunkProver, Verifier as ChunkVerifier},
    BatchProvingTask, ChunkProvingTask,
};
use std::{env, time::Instant};

/// The `output_dir` is assumed to output_dir of chunk proving.
pub fn new_batch_prover(output_dir: &str) -> BatchProver {
    env::set_var("CHUNK_PROTOCOL_FILENAME", "chunk_chunk_0.protocol");
    let prover = BatchProver::from_dirs(PARAMS_DIR, output_dir);
    log::info!("Constructed batch prover");

    prover
}

pub fn prove_and_verify_chunk(
    chunk: ChunkProvingTask,
    chunk_identifier: Option<&str>,
    params_path: &str,
    assets_path: &str,
    output_dir: &str,
) {
    let mut prover = ChunkProver::from_dirs(params_path, assets_path);
    log::info!("Constructed chunk prover");

    let now = Instant::now();
    let chunk_proof = prover
        .gen_chunk_proof(chunk, chunk_identifier, None, Some(output_dir))
        .expect("cannot generate chunk snark");
    log::info!(
        "finish generating chunk snark, elapsed: {:?}",
        now.elapsed()
    );

    // output_dir is used to load chunk vk
    env::set_var("CHUNK_VK_FILENAME", "vk_chunk_0.vkey");
    let verifier = ChunkVerifier::from_dirs(params_path, output_dir);
    assert!(verifier.verify_chunk_proof(chunk_proof));
}

pub fn prove_and_verify_batch(
    output_dir: &str,
    batch_prover: &mut BatchProver,
    batch: BatchProvingTask,
) {
    let chunk_num = batch.chunk_proofs.len();
    log::info!("Prove batch BEGIN: chunk_num = {chunk_num}");

    let batch_proof = batch_prover
        .gen_agg_evm_proof(batch, None, Some(output_dir))
        .unwrap();

    env::set_var("AGG_VK_FILENAME", "vk_batch_agg.vkey");
    let verifier = BatchVerifier::from_dirs(PARAMS_DIR, output_dir);
    log::info!("Constructed aggregator verifier");

    assert!(verifier.verify_agg_evm_proof(batch_proof));
    log::info!("Verified batch proof");

    log::info!("Prove batch BEGIN: chunk_num = {}", chunk_num);
}
