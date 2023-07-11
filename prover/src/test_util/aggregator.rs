use crate::{
    aggregator::Prover,
    io::{load_snark, write_snark},
    utils::gen_rng,
    zkevm::circuit::SuperCircuit,
    Proof,
};
use aggregator::ChunkHash;
use halo2_proofs::halo2curves::bn256::Fr;
use snark_verifier_sdk::Snark;
use std::{env::set_var, path::PathBuf};
use zkevm_circuits::evm_circuit::witness::Block;

pub fn load_or_gen_agg_snark(
    output_dir: &str,
    id: &str,
    degree: u32,
    prover: &mut Prover,
    chunk_hashes: &[ChunkHash],
    prev_snarks: &[Snark],
) -> Snark {
    set_var("VERIFY_CONFIG", format!("./configs/{id}.config"));
    let file_path = format!("{output_dir}/{id}_snark.json");

    load_snark(&file_path).unwrap().unwrap_or_else(|| {
        let rng = gen_rng();
        let snark = prover.gen_agg_snark(id, degree, rng, chunk_hashes, prev_snarks);
        write_snark(&file_path, &snark);

        snark
    })
}

pub fn gen_comp_evm_proof(
    output_dir: &str,
    id: &str,
    is_fresh: bool,
    degree: u32,
    prover: &mut Prover,
    prev_snark: Snark,
) -> Proof {
    // gupeng
    set_var("VERIFY_CONFIG", format!("./configs/{id}.config"));
    set_var("COMPRESSION_CONFIG", format!("./configs/{id}.config"));

    let rng = gen_rng();
    let proof = prover
        .gen_comp_evm_proof(id, is_fresh, degree, rng, prev_snark)
        .unwrap();
    proof.dump(&mut PathBuf::from(output_dir), id).unwrap();

    proof
}

pub fn load_or_gen_chunk_snark(
    output_dir: &str,
    id: &str,
    prover: &mut Prover,
    witness_block: Block<Fr>,
) -> Snark {
    let file_path = format!("{output_dir}/{id}_chunk_snark.json");

    load_snark(&file_path).unwrap().unwrap_or_else(|| {
        let snark = prover
            .gen_chunk_snark::<SuperCircuit>(&witness_block)
            .unwrap();
        write_snark(&file_path, &snark);

        snark
    })
}

pub fn load_or_gen_comp_snark(
    output_dir: &str,
    id: &str,
    is_fresh: bool,
    degree: u32,
    prover: &mut Prover,
    prev_snark: Snark,
) -> Snark {
    // gupeng
    set_var("VERIFY_CONFIG", format!("./configs/{id}.config"));
    set_var("COMPRESSION_CONFIG", format!("./configs/{id}.config"));
    let file_path = format!("{output_dir}/{id}_snark.json");

    load_snark(&file_path).unwrap().unwrap_or_else(|| {
        let rng = gen_rng();
        let snark = prover
            .gen_comp_snark(id, is_fresh, degree, rng, prev_snark)
            .unwrap();
        write_snark(&file_path, &snark);

        snark
    })
}
