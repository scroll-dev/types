use crate::utils::{c_char_to_str, c_char_to_vec, vec_to_c_char, OUTPUT_DIR};
use libc::c_char;
use prover::{
    aggregator::{Prover, Verifier},
    io::read_all,
    utils::{chunk_trace_to_witness_block, init_env_and_log},
    ChunkHash, ChunkProof, Proof,
};
use std::cell::OnceCell;
use types::eth::BlockTrace;

static mut PROVER: OnceCell<Prover> = OnceCell::new();
static mut VERIFIER: OnceCell<Verifier> = OnceCell::new();

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn init_agg_prover(params_dir: *const c_char) {
    init_env_and_log("ffi_agg_prove");

    let params_dir = c_char_to_str(params_dir);
    let prover = Prover::from_params_dir(params_dir);

    PROVER.set(prover).unwrap();
}

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn init_agg_verifier(params_dir: *const c_char, vk_path: *const c_char) {
    init_env_and_log("ffi_agg_verify");

    let params_dir = c_char_to_str(params_dir);
    let raw_vk = read_all(c_char_to_str(vk_path));
    let verifier = Verifier::from_params_dir(params_dir, &raw_vk);

    VERIFIER.set(verifier).unwrap();
}

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn gen_agg_proof(
    chunk_hashes: *const c_char,
    chunk_proofs: *const c_char,
) -> *const c_char {
    let chunk_hashes = c_char_to_vec(chunk_hashes);
    let chunk_proofs = c_char_to_vec(chunk_proofs);

    let chunk_hashes = serde_json::from_slice::<Vec<ChunkHash>>(&chunk_hashes).unwrap();
    let chunk_proofs = serde_json::from_slice::<Vec<ChunkProof>>(&chunk_proofs).unwrap();
    assert_eq!(chunk_hashes.len(), chunk_proofs.len());

    let chunk_hashes_proofs = chunk_hashes
        .into_iter()
        .zip(chunk_proofs.into_iter())
        .collect();

    let proof = PROVER
        .get_mut()
        .unwrap()
        .gen_agg_proof(chunk_hashes_proofs, None, OUTPUT_DIR.as_deref())
        .unwrap();

    let proof_bytes = serde_json::to_vec(&proof).unwrap();
    vec_to_c_char(proof_bytes)
}

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn verify_agg_proof(proof: *const c_char) -> c_char {
    let proof = c_char_to_vec(proof);
    let proof = serde_json::from_slice::<Proof>(proof.as_slice()).unwrap();

    let verified = VERIFIER.get().unwrap().verify_agg_proof(proof);
    verified as c_char
}

// This function is only used for debugging on Go side.
/// # Safety
#[no_mangle]
pub unsafe extern "C" fn block_traces_to_chunk_hash(block_traces: *const c_char) -> *const c_char {
    let block_traces = c_char_to_vec(block_traces);
    let block_traces = serde_json::from_slice::<Vec<BlockTrace>>(&block_traces).unwrap();

    let witness_block = chunk_trace_to_witness_block(block_traces).unwrap();
    let chunk_hash = ChunkHash::from_witness_block(&witness_block, false);

    let chunk_hash_bytes = serde_json::to_vec(&chunk_hash).unwrap();
    vec_to_c_char(chunk_hash_bytes)
}
