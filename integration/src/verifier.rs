use std::path::PathBuf;

use halo2_proofs::{halo2curves::bn256::Bn256, poly::kzg::commitment::ParamsKZG};
use prover::{batch_vk_filename, CompressionCircuit, Verifier};
use std::{collections::BTreeMap, env};

use prover::{force_read, DEPLOYMENT_CODE_FILENAME};

type SnarkVerifier<'a> = Verifier<'a, CompressionCircuit>;

// FIXME: why we use common::Verifier instead of ChunkVerifier here?
pub fn new_chunk_verifier<'a>(
    params_map: &'a BTreeMap<u32, ParamsKZG<Bn256>>,
    assets_dir: &str,
) -> SnarkVerifier<'a> {
    let path = std::path::PathBuf::from(assets_dir).join(prover::chunk_vk_filename());
    let raw_vk = force_read(&path);
    if raw_vk.is_empty() {
        panic!("empty vk read from {path:?}");
    }
    env::set_var("COMPRESSION_CONFIG", &*prover::LAYER2_CONFIG_PATH);
    let params = params_map
        .get(&prover::LAYER2_DEGREE)
        .expect("should be loaded");
    SnarkVerifier::from_params(params, &raw_vk)
}

#[allow(dead_code)]
pub fn new_batch_verifier<'a>(
    params_map: &'a BTreeMap<u32, ParamsKZG<Bn256>>,
    assets_dir: &str,
) -> SnarkVerifier<'a> {
    let path = PathBuf::from(assets_dir).join(batch_vk_filename());
    let raw_vk = force_read(&path);
    if raw_vk.is_empty() {
        panic!("empty vk read from {path:?}");
    }
    env::set_var("COMPRESSION_CONFIG", &*prover::LAYER4_CONFIG_PATH);
    let params = params_map
        .get(&prover::LAYER4_DEGREE)
        .expect("should be loaded");
    SnarkVerifier::from_params(params, &raw_vk)
}

#[derive(Debug)]
pub struct EVMVerifier(Vec<u8>);

impl EVMVerifier {
    pub fn new(deployment_code: Vec<u8>) -> Self {
        Self(deployment_code)
    }

    pub fn from_dirs(assets_dir: &str) -> Self {
        let path = PathBuf::from(assets_dir).join(DEPLOYMENT_CODE_FILENAME.clone());
        Self::new(force_read(&path))
    }

    pub fn verify_evm_proof(&self, call_data: Vec<u8>) -> bool {
        prover::deploy_and_call(self.0.clone(), call_data).is_ok()
    }
}
