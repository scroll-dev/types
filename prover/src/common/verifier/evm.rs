use super::Verifier;
use crate::{io::write_file, EvmProof, Proof};
use halo2_proofs::halo2curves::bn256::{Bn256, Fr};
use snark_verifier::pcs::kzg::{Bdfg21, Kzg};
use snark_verifier_sdk::{gen_evm_verifier, verify_evm_proof, CircuitExt};
use std::{path::PathBuf, str::FromStr};

impl<C: CircuitExt<Fr>> Verifier<C> {
    // Should panic if failed to verify.
    pub fn evm_verify(&self, evm_proof: &EvmProof, output_dir: &str) {
        let mut yul_file_path = PathBuf::from_str(output_dir).unwrap();
        yul_file_path.push("evm_verifier.yul");

        // Generate deployment code and dump YUL file.
        let deployment_code = gen_evm_verifier::<C, Kzg<Bn256, Bdfg21>>(
            &self.params,
            &self.vk,
            evm_proof.num_instance.clone(),
            Some(yul_file_path.as_path()),
        );

        // Dump bytecode.
        let mut output_dir = PathBuf::from_str(output_dir).unwrap();
        write_file(&mut output_dir, "evm_verifier.bin", &deployment_code);

        let success = self.verify_evm_proof(deployment_code, &evm_proof.proof);
        assert!(success);
    }

    pub fn verify_evm_proof(&self, deployment_code: Vec<u8>, proof: &Proof) -> bool {
        let proof_data = proof.proof().to_vec();
        verify_evm_proof(deployment_code, proof.instances(), proof_data)
    }
}
