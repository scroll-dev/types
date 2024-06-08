use integration::test_util::{prepare_circuit_capacity_checker, run_circuit_capacity_checker};
use prover::{utils::init_env_and_log, BlockTrace};
use std::env;

mod l2geth_client;
mod prove_utils;
mod rollupscan_client;

const DEFAULT_BEGIN_BATCH: i64 = 1;
const DEFAULT_END_BATCH: i64 = i64::MAX;

#[tokio::main]
async fn main() {
    init_env_and_log("chain_prover");

    log::info!("chain_prover: BEGIN");

    let setting = Setting::new();
    log::info!("chain_prover: setting = {setting:?}");

    prepare_circuit_capacity_checker();
    log::info!("chain_prover: prepared ccc");

    let l2geth = l2geth_client::Client::new("chain_prover", &setting.l2geth_api_url)
        .unwrap_or_else(|e| panic!("chain_prover: failed to initialize ethers Provider: {e}"));
    let rollupscan = rollupscan_client::Client::new("chain_prover", &setting.rollupscan_api_url);

    for batch_id in setting.begin_batch..=setting.end_batch {
        let chunks = rollupscan
            .get_chunk_info_by_batch_index(batch_id)
            .await
            .unwrap_or_else(|e| {
                panic!("chain_prover: failed to request rollupscan chunks API for batch-{batch_id}: {e}")
            });

        if chunks.is_none() {
            log::warn!("chain_prover: no chunks in batch-{batch_id}");
            continue;
        }

        let mut chunk_proofs = vec![];
        for chunk in chunks.unwrap() {
            let chunk_id = chunk.index;
            log::info!("chain_prover: handling chunk {:?}", chunk_id);

            let mut block_traces: Vec<BlockTrace> = vec![];
            for block_num in chunk.start_block_number..=chunk.end_block_number {
                let trace = l2geth
                    .get_block_trace_by_num(block_num)
                    .await
                    .unwrap_or_else(|e| {
                        panic!("chain_prover: failed to request l2geth block-trace API for batch-{batch_id} chunk-{chunk_id} block-{block_num}: {e}")
                    });

                block_traces.push(trace);
            }

            if env::var("CIRCUIT").unwrap_or_default() == "ccc" {
                run_circuit_capacity_checker(batch_id, chunk_id, &block_traces);
                continue;
            }

            let chunk_proof = prove_utils::prove_chunk(
                &format!("chain_prover: batch-{batch_id} chunk-{chunk_id}"),
                block_traces,
            );

            if let Some(chunk_proof) = chunk_proof {
                chunk_proofs.push(chunk_proof);
            }
        }

        #[cfg(feature = "batch-prove")]
        prove_utils::prove_batch(&format!("chain_prover: batch-{batch_id}"), chunk_proofs);
    }

    log::info!("chain_prover: END");
}

#[derive(Debug)]
struct Setting {
    begin_batch: i64,
    end_batch: i64,
    l2geth_api_url: String,
    rollupscan_api_url: String,
}

impl Setting {
    pub fn new() -> Self {
        let l2geth_api_url =
            env::var("L2GETH_API_URL").expect("chain_prover: Must set env L2GETH_API_URL");
        let rollupscan_api_url = env::var("ROLLUPSCAN_API_URL");
        let rollupscan_api_url =
            rollupscan_api_url.unwrap_or_else(|_| "http://10.0.3.119:8560/api/chunks".to_string());
        let begin_batch = env::var("PROVE_BEGIN_BATCH")
            .ok()
            .and_then(|n| n.parse().ok())
            .unwrap_or(DEFAULT_BEGIN_BATCH);
        let end_batch = env::var("PROVE_END_BATCH")
            .ok()
            .and_then(|n| n.parse().ok())
            .unwrap_or(DEFAULT_END_BATCH);

        Self {
            begin_batch,
            end_batch,
            l2geth_api_url,
            rollupscan_api_url,
        }
    }
}
