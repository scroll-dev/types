use super::{dump_as_json, dump_vk, from_json_file, Proof};
use anyhow::Result;
use halo2_proofs::{halo2curves::bn256::G1Affine, plonk::ProvingKey};
use serde_derive::{Deserialize, Serialize};
use snark_verifier::Protocol;
use snark_verifier_sdk::Snark;
use types::{base64, eth::StorageTrace};

#[derive(Debug, Deserialize, Serialize)]
pub struct ChunkProof {
    #[serde(with = "base64")]
    pub storage_trace: Vec<u8>,
    #[serde(with = "base64")]
    pub protocol: Vec<u8>,
    #[serde(flatten)]
    pub proof: Proof,
}

impl ChunkProof {
    pub fn new(
        snark: Snark,
        storage_trace: StorageTrace,
        pk: Option<&ProvingKey<G1Affine>>,
    ) -> Result<Self> {
        let storage_trace = serde_json::to_vec(&storage_trace)?;
        let protocol = serde_json::to_vec(&snark.protocol)?;
        let proof = Proof::new(snark.proof, &snark.instances, pk)?;

        Ok(Self {
            storage_trace,
            protocol,
            proof,
        })
    }

    pub fn from_json_file(dir: &str, name: &str) -> Result<Self> {
        from_json_file(dir, &dump_filename(name))
    }

    pub fn dump(&self, dir: &str, name: &str) -> Result<()> {
        let filename = dump_filename(name);

        dump_vk(dir, &filename, &self.proof.vk);
        dump_as_json(dir, &filename, &self)
    }

    pub fn to_snark_and_storage_trace(self) -> (Snark, StorageTrace) {
        let instances = self.proof.instances();
        let storage_trace = serde_json::from_slice::<StorageTrace>(&self.storage_trace).unwrap();
        let protocol = serde_json::from_slice::<Protocol<G1Affine>>(&self.protocol).unwrap();

        (
            Snark {
                protocol,
                proof: self.proof.proof,
                instances,
            },
            storage_trace,
        )
    }
}

fn dump_filename(name: &str) -> String {
    format!("chunk_{name}")
}
