use prover::{eth_types::l2_types::BlockTrace, read_env_var};
use serde_json::Result as SeResult;
use sp1_host::{trace, BlockTrace as SbvBlockTrace};
use std::io::{Read, Error};

pub fn to_sp1_block_trace(block_trace: &BlockTrace) -> SeResult<SbvBlockTrace> {
    // TODO: there would be a huge work to turn each member in `block_trace`
    // to corresponding one in the sp1 struct since they are all derived
    // by alloy. A serialize - deserialize process is induced for workaround
    serde_json::from_slice(&serde_json::to_vec(block_trace)?)
}

pub struct ToSp1BlockTrace<'a>(pub &'a BlockTrace);

impl<'a> TryInto<trace::BlockTrace> for ToSp1BlockTrace<'a> {
    type Error = serde_json::Error;

    fn try_into(self) -> Result<trace::BlockTrace, Self::Error> {
        to_sp1_block_trace(self.0).map(trace::BlockTrace)
    }
}

pub fn load_elf() -> Result<Vec<u8>, Error> {
    let sp1_evm_path =
        read_env_var("SP1EVM_PATH", "elf/riscv32im-succinct-zkvm-elf".to_string());

    let mut buffer = Vec::new();
    std::fs::File::open(sp1_evm_path)?.read_to_end(&mut buffer)?;

    Ok(buffer)
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_to_sp1_trace() {
        let json_bytes = include_bytes!(
            "../../integration/tests/extra_traces/batch1/chunk_1/block_7156762.json"
        );
        let trace = serde_json::from_slice::<BlockTrace>(json_bytes).unwrap();

        let sp1_trace = to_sp1_block_trace(&trace).unwrap();

        // randomly check some fields
        assert_eq!(
            format!("{:?}", trace.coinbase.address),
            sp1_trace.coinbase.address.to_string(),
        );
        assert_eq!(
            format!("{}", trace.header.number.unwrap()),
            sp1_trace.header.number.to_string(),
        );
        assert_eq!(
            format!("{:?}", trace.header.hash.unwrap()),
            sp1_trace.header.hash.to_string(),
        );
        assert_eq!(
            format!("{:?}", trace.transactions[0].tx_hash),
            sp1_trace.transactions[0].tx_hash.to_string(),
        );
    }
}
