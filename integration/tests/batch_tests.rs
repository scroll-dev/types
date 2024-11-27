use integration::prove::{new_batch_prover, prove_and_verify_batch};
use prover::{init_env_and_log, read_json_deep, BatchProvingTask};
use std::{fs, path::PathBuf};

#[cfg(feature = "prove_verify")]
#[test]
fn test_batch_prove_verify() {
    use integration::test_util::PARAMS_DIR;
    use itertools::Itertools;
    use prover::BATCH_PROVER_DEGREES;

    let output_dir = init_env_and_log("batch_tests");
    log::info!("Initialized ENV and created output-dir {output_dir}");

    let params_map = prover::Prover::load_params_map(
        PARAMS_DIR,
        &BATCH_PROVER_DEGREES.iter().copied().collect_vec(),
    );

    //let task_path = "tests/test_data/batch-task-with-blob.json"; // zstd
    let task_path = "tests/test_data/batch-task-with-blob-raw.json"; // no zstd
    let mut batch = load_batch_proving_task(task_path);
    log::info!("batch hash = {:?}", batch.batch_header.batch_hash());

    let chunk_infos = batch
        .chunk_proofs
        .clone()
        .into_iter()
        .map(|p| p.inner.chunk_info().clone())
        .collect::<Vec<_>>();
    let corrected_batch_header = prover::BatchHeader::construct_from_chunks(
        batch.batch_header.version,
        batch.batch_header.batch_index,
        batch.batch_header.l1_message_popped,
        batch.batch_header.total_l1_message_popped,
        batch.batch_header.parent_batch_hash,
        batch.batch_header.last_block_timestamp,
        &chunk_infos,
        &batch.blob_bytes,
    );
    batch.batch_header = corrected_batch_header;

    dump_chunk_protocol(&batch, &output_dir);
    let mut batch_prover = new_batch_prover(&params_map, &output_dir);
    prove_and_verify_batch(&params_map, &output_dir, &mut batch_prover, batch);
}

#[cfg(feature = "prove_verify")]
#[test]
fn test_batches_with_each_chunk_num_prove_verify() {
    use integration::test_util::PARAMS_DIR;
    use itertools::Itertools;

    let output_dir = init_env_and_log("batches_with_each_chunk_num_tests");
    log::info!("Initialized ENV and use output-dir {output_dir}");

    let params_map = prover::Prover::load_params_map(
        PARAMS_DIR,
        &prover::BATCH_PROVER_DEGREES.iter().copied().collect_vec(),
    );

    let batch = load_batch_proving_task("tests/test_data/full_proof_1.json");
    dump_chunk_protocol(&batch, &output_dir);
    let mut batch_prover = new_batch_prover(&params_map, &output_dir);

    // Iterate over chunk proofs to test with 1 to max chunks (in a batch).
    for len in 1..batch.chunk_proofs.len() {
        let mut output_dir = PathBuf::from(&output_dir);
        output_dir.push(format!("batch_{}", len));
        fs::create_dir_all(&output_dir).unwrap();
        let batch = BatchProvingTask {
            batch_header: batch.batch_header,
            chunk_proofs: batch.chunk_proofs[..len].to_vec(),
            // FIXME
            blob_bytes: vec![],
        };
        prove_and_verify_batch(
            &params_map,
            &output_dir.to_string_lossy(),
            &mut batch_prover,
            batch,
        );
    }
}

#[cfg(feature = "prove_verify")]
#[ignore = "only used for subsequent chunk tests"]
#[test]
fn test_batch_prove_verify_after_chunk_tests() {
    use integration::{
        prove::get_blob_from_chunks,
        test_util::{load_chunk, trace_path_for_test, PARAMS_DIR},
    };
    use itertools::Itertools;
    use prover::{
        eth_types::H256, BatchHeader, ChunkProofV2, ChunkProvingTask, BATCH_PROVER_DEGREES,
    };

    let output_dir = init_env_and_log("batch_tests");
    log::info!("Initialized ENV and created output-dir {output_dir}");

    let params_map = prover::Prover::load_params_map(
        PARAMS_DIR,
        &BATCH_PROVER_DEGREES.iter().copied().collect_vec(),
    );

    let trace_paths_env = trace_path_for_test();
    let trace_paths: Vec<_> = trace_paths_env.split(';').collect();
    log::info!("Use traces paths {trace_paths:?}");

    let mut l1_message_popped = 0;
    let mut last_block_timestamp = 0;

    // like gen_batch_proving_task in e2e, collect every chunks
    let chunk_proofs = trace_paths
        .iter()
        .map(|chunk_dir| load_chunk(chunk_dir).1)
        .map(|traces| {
            // make use of traces before consumed by chunkproof
            l1_message_popped += traces.iter().map(|tr| tr.num_l1_txs()).sum::<u64>();
            last_block_timestamp = traces
                .last()
                .map_or(last_block_timestamp, |tr| tr.header.timestamp.as_u64());

            let task = ChunkProvingTask::new(traces);
            let loaded_proof = ChunkProofV2::from_json(&output_dir, &task.identifier());
            if let Ok(proof) = loaded_proof.as_ref() {
                log::info!(
                    "expected PI of {} is {:#x?}",
                    task.identifier(),
                    proof.inner.chunk_info().public_input_hash(),
                );
            }
            loaded_proof
        })
        .collect::<Result<Vec<_>, _>>()
        .unwrap();

    let chunk_infos = chunk_proofs
        .iter()
        .map(|proof| proof.inner.chunk_info().clone())
        .collect::<Vec<_>>();

    let blob_bytes = get_blob_from_chunks(&chunk_infos);

    let batch_header = BatchHeader::construct_from_chunks(
        4,
        123,
        l1_message_popped,
        l1_message_popped,
        H256([
            0xab, 0xac, 0xad, 0xae, 0xaf, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0,
        ]),
        last_block_timestamp,
        &chunk_infos,
        &blob_bytes,
    );

    let batch = BatchProvingTask {
        chunk_proofs,
        batch_header,
        blob_bytes,
    };

    // dump_chunk_protocol(&batch, &output_dir);
    let mut batch_prover = new_batch_prover(&params_map, &output_dir);
    prove_and_verify_batch(&params_map, &output_dir, &mut batch_prover, batch);
}

fn load_batch_proving_task(batch_task_file: &str) -> BatchProvingTask {
    let batch: BatchProvingTask = read_json_deep(batch_task_file).unwrap();
    let tx_bytes_total_len: usize = batch
        .chunk_proofs
        .iter()
        .map(|c| c.inner.chunk_info().tx_bytes.len())
        .sum();
    log::info!("Loaded chunk-hashes and chunk-proofs, batch info: chunk num {}, tx_bytes_total_len {tx_bytes_total_len}", batch.chunk_proofs.len());
    batch
}

fn dump_chunk_protocol(batch: &BatchProvingTask, output_dir: &str) {
    // Dump chunk-procotol to "chunk_chunk_0.protocol" for batch proving.
    batch
        .chunk_proofs
        .first()
        .unwrap()
        .dump(output_dir, "0")
        .unwrap();
}
