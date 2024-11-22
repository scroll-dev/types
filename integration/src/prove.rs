use halo2_proofs::{halo2curves::bn256::Bn256, poly::kzg::commitment::ParamsKZG};
use prover::{
    chunk_trace_to_witness_block, get_blob_bytes, BatchData, BatchProofV2, BatchProver,
    BatchProvingTask, BatchVerifier, BundleProvingTask, ChunkInfo, ChunkProofV2,
    ChunkProofV2Metadata, ChunkProver, ChunkProvingTask, ChunkVerifier, Snark, MAX_AGG_SNARKS,
    Sp1Prover,
};
use std::{collections::BTreeMap, env, time::Instant};

use crate::verifier::{new_chunk_verifier, EVMVerifier};

/// The `output_dir` is assumed to output_dir of chunk proving.
pub fn new_batch_prover<'a>(
    params_map: &'a BTreeMap<u32, ParamsKZG<Bn256>>,
    output_dir: &str,
) -> BatchProver<'a> {
    env::set_var("HALO2_CHUNK_PROTOCOL", "protocol_chunk_halo2.protocol");
    env::set_var("SP1_CHUNK_PROTOCOL", "protocol_chunk_sp1.protocol");
    env::set_var("SCROLL_PROVER_ASSETS_DIR", output_dir);
    let prover = BatchProver::from_params_and_assets(params_map, output_dir);
    log::info!("Constructed batch prover");

    prover
}

/// prove_and_verify_sp1_chunk would expect a sp1 snark name "sp1_snark_<chunk_id>.json"
pub fn prove_and_verify_sp1_chunk(
    params_map: &BTreeMap<u32, ParamsKZG<Bn256>>,
    output_dir: &str,
    chunk: ChunkProvingTask,
    prover: &mut Sp1Prover,
    chunk_identifier: Option<&str>,
) -> ChunkProofV2 {
    let chunk_identifier =
        chunk_identifier.map_or_else(|| chunk.identifier(), |name| name.to_string());

    let now = Instant::now();
    let chunk_proof = prover
        .gen_chunk_proof(chunk, Some(&chunk_identifier), None, Some(output_dir))
        .expect("cannot generate chunk snark");
    log::info!(
        "finish generating chunk snark, elapsed: {:?}",
        now.elapsed()
    );

    // output_dir is used to load chunk vk
    env::set_var(
        "CHUNK_VK_FILENAME",
        &format!("vk_chunk_{chunk_identifier}.vkey"),
    );
    let verifier = new_chunk_verifier(params_map, output_dir);
    let snark = Snark::try_from(&chunk_proof).expect("should be ok");
    assert!(verifier.verify_snark(snark));
    log::info!("Verified sp1 chunk proof");

    chunk_proof
}

pub fn prove_and_verify_chunk(
    params_map: &BTreeMap<u32, ParamsKZG<Bn256>>,
    output_dir: &str,
    chunk: ChunkProvingTask,
    prover: &mut ChunkProver,
    chunk_identifier: Option<&str>,
    skip_verify: bool,
) -> ChunkProofV2 {
    let chunk_identifier =
        chunk_identifier.map_or_else(|| chunk.identifier(), |name| name.to_string());

    let now = Instant::now();
    let chunk_proof = prover
        .gen_halo2_chunk_proof(chunk, Some(&chunk_identifier), None, Some(output_dir))
        .expect("cannot generate chunk snark");
    log::info!(
        "finish generating chunk snark, elapsed: {:?}",
        now.elapsed()
    );

    // there is an issue: if snark is restore from disk, the pk is not generated
    // and the dumping process of proof would write the existed vk with 0 bytes
    // and cause verify failed
    // the work-around is skip verify in e2e test
    if skip_verify {
        return chunk_proof;
    }
    // output_dir is used to load chunk vk
    env::set_var(
        "CHUNK_VK_FILENAME",
        &format!("vk_chunk_{chunk_identifier}.vkey"),
    );
    let verifier = ChunkVerifier::from_params_and_assets(params_map, output_dir);
    verifier
        .verify_chunk_proof(&chunk_proof)
        .expect("should verify");
    log::info!("Verified chunk proof");

    chunk_proof
}

pub fn prove_and_verify_batch(
    params_map: &BTreeMap<u32, ParamsKZG<Bn256>>,
    output_dir: &str,
    batch_prover: &mut BatchProver,
    batch: BatchProvingTask,
) -> BatchProofV2 {
    let chunk_num = batch.chunk_proofs.len();
    log::info!("Prove batch BEGIN: chunk_num = {chunk_num}");

    let batch_id = batch.identifier();
    let res_batch_proof = batch_prover.gen_batch_proof(batch, None, Some(output_dir));
    if let Err(e) = res_batch_proof {
        log::error!("proving err: {e}");
        panic!("proving err: {:?}", e);
    }
    let batch_proof = res_batch_proof.unwrap();

    env::set_var("BATCH_VK_FILENAME", format!("vk_batch_{batch_id}.vkey"));
    let verifier = BatchVerifier::from_params_and_assets(params_map, output_dir);
    log::info!("Constructed aggregator verifier");

    verifier
        .verify_batch_proof(&batch_proof)
        .expect("should verify");
    log::info!("Verified batch proof");

    log::info!("Prove batch END: chunk_num = {chunk_num}");

    batch_proof
}

pub fn prove_and_verify_bundle(
    output_dir: &str,
    prover: &mut BatchProver,
    bundle: BundleProvingTask,
) {
    log::info!("Prove bundle BEGIN");

    let bundle_proof = prover
        .gen_bundle_proof(bundle, None, Some(output_dir))
        .unwrap();

    env::set_var("BATCH_VK_FILENAME", "vk_bundle_recursion.vkey");
    let verifier = EVMVerifier::from_dirs(output_dir);
    log::info!("Constructed bundle verifier");

    assert!(verifier.verify_evm_proof(bundle_proof.calldata()));
    log::info!("Verifier bundle proof");

    log::info!("Prove bundle END");
}

// `chunks` are unpadded
// Similar codes with aggregator/src/tests/aggregation.rs
// Refactor?
pub fn get_blob_from_chunks(chunks: &[ChunkInfo]) -> Vec<u8> {
    let num_chunks = chunks.len();

    let padded_chunk =
        ChunkInfo::mock_padded_chunk_info_for_testing(chunks.last().as_ref().unwrap());
    let chunks_with_padding = [
        chunks.to_vec(),
        vec![padded_chunk; MAX_AGG_SNARKS - num_chunks],
    ]
    .concat();
    let batch_data = BatchData::<{ MAX_AGG_SNARKS }>::new(chunks.len(), &chunks_with_padding);
    let batch_bytes = batch_data.get_batch_data_bytes();
    let blob_bytes = get_blob_bytes(&batch_bytes);
    log::info!("blob_bytes len {}", blob_bytes.len());
    blob_bytes
}
