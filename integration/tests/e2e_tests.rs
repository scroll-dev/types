use halo2_proofs::{halo2curves::bn256::Bn256, poly::kzg::commitment::ParamsKZG};
use integration::{
    prove::{get_blob_from_chunks, prove_and_verify_chunk, prove_and_verify_sp1_chunk},
    test_util::{load_batch, load_chunk, ASSETS_DIR, PARAMS_DIR},
};
use prover::{
    eth_types::H256, init_env_and_log, read_env_var, BatchHeader, BatchProvingTask, ChunkProver, Sp1Prover,
    ChunkProvingTask, MAX_AGG_SNARKS,
};
use std::collections::BTreeMap;

/*
#[cfg(feature = "fix_later")]
#[test]
fn test_batch_pi_consistency() {
    let output_dir = init_env_and_log("batch_pi");
    log::info!("Initialized ENV and created output-dir {output_dir}");

    let batch_dir = prover::read_env_var("TRACE_PATH", "./tests/extra_traces/batch_25".to_string());
    let trace_paths = load_batch(&batch_dir).unwrap();
    log_batch_pi(&trace_paths);
}
*/

#[cfg(feature = "prove_verify")]
#[test]
fn test_e2e_prove_verify() {
    use integration::prove::{new_batch_prover, prove_and_verify_batch, prove_and_verify_bundle};
    use itertools::Itertools;
    use prover::{dump_as_json, BATCH_PROVER_DEGREES, CHUNK_PROVER_DEGREES};

    let output_dir = init_env_and_log("e2e_tests");
    log::info!("Initialized ENV and created output-dir {output_dir}");

    let params_map = prover::Prover::load_params_map(
        PARAMS_DIR,
        &CHUNK_PROVER_DEGREES
            .iter()
            .copied()
            .chain(BATCH_PROVER_DEGREES.iter().copied())
            .collect_vec(),
    );

    let chunk_paths = read_env_var(
        "E2E_TRACE_PATHS",
        "./tests/extra_traces/batch1;./tests/extra_traces/batch2".to_string(),
    );
    let chunks = chunk_paths.split(';').map(|dir| load_batch(dir).unwrap());

    let mut batch_prover_pending = None;
    let mut opt_batch_header = None;
    let mut batch_proofs = Vec::new();

    for (i, chunk) in chunks.enumerate() {
        let (batch, batch_header) = gen_batch_proving_task(
            &params_map,
            &output_dir,
            &chunk,
            opt_batch_header,
        );
        dump_as_json(
            &output_dir,
            format!("batch_prove_{}", i + 1).as_str(),
            &batch,
        )
        .unwrap();
        if i == 0 {
            dump_chunk_protocol(&batch, &output_dir);
            batch_prover_pending.replace(new_batch_prover(&params_map, &output_dir));
        }
        let batch_prover = batch_prover_pending.as_mut().unwrap();

        let batch_proof = prove_and_verify_batch(&params_map, &output_dir, batch_prover, batch);
        /*
        use std::{fs, path::Path};
        let proof_path = Path::new(&output_dir).join("full_proof_batch_agg.json");
        let proof_path_to =
            Path::new(&output_dir).join(format!("full_proof_batch_agg_{}.json", i + 1).as_str());
        fs::rename(proof_path, proof_path_to).unwrap();
        */
        log::info!(
            "batch proof {}, prev hash {:x?}, current {:x?}",
            i,
            batch_header.parent_batch_hash,
            batch_proof.inner.batch_hash,
        );
        opt_batch_header.replace(batch_header);
        batch_proofs.push(batch_proof);
    }

    let batch_prover = batch_prover_pending.as_mut().unwrap();
    let bundle = prover::BundleProvingTask { batch_proofs };
    prove_and_verify_bundle(&output_dir, batch_prover, bundle);
}

#[cfg(feature = "prove_verify")]
#[test]
fn test_e2e_prove_verify_hybrid() {
    use integration::prove::{new_batch_prover, prove_and_verify_batch, prove_and_verify_bundle};
    use itertools::Itertools;
    use prover::{BATCH_PROVER_DEGREES, CHUNK_PROVER_DEGREES};

    // setup output dir for tests.
    let output_dir = init_env_and_log("e2e_tests");
    log::info!("initialize ENV and create output-dir {output_dir}: OK");

    // load kzg params.
    let params_map = prover::Prover::load_params_map(
        PARAMS_DIR,
        &CHUNK_PROVER_DEGREES
            .iter()
            .copied()
            .chain(BATCH_PROVER_DEGREES.iter().copied())
            .collect_vec(),
    );
    log::info!("load params map: OK");

    // - batch1: contains chunk_1 (halo2-based super circuit).
    // - batch_sproll: contains chunk_1 (sp1). This dir already contains inner snark.
    let chunk_paths = read_env_var(
        "E2E_TRACE_PATHS",
        "./tests/extra_traces/hybrid_batch/".to_string(),
    );
    let chunks = chunk_paths
        .split(';')
        .map(|dir| load_batch(dir).unwrap())
        .collect_vec();
    log::info!("read chunks paths");
    for (i, item) in chunks.iter().enumerate() {
        log::info!("{i}: {item:?}");
    }
    assert_eq!(chunks.len(), 1, "there is 1 batch");
    assert_eq!(chunks[0].len(), 2, "there are 2 chunks in the batch");
    log::info!("OK");

    // // path to find inner snark generated via sp1 and sp1-halo2-wrap.
    // let sp1_path = {
    //     let p = read_env_var("SP1_PATH", String::new());
    //     if p.is_empty() {
    //         None
    //     } else {
    //         Some(p)
    //     }
    // };
    // assert!(sp1_path.is_some(), "must provide SP1_PATH env var");
    // log::info!("sp1 path: {sp1_path:?} OK");

    let chunk_1 = load_chunk(&chunks[0][0]).1;
    let chunk_2 = load_chunk(&chunks[0][1]).1;
    let l1_message_popped = [chunk_1.clone(), chunk_2.clone()]
        .iter()
        .flatten()
        .map(|chunk| chunk.num_l1_txs())
        .sum();
    let last_block_timestamp = chunk_2
        .last()
        .map_or(0, |block_trace| block_trace.header.timestamp.as_u64());

    // halo2 chunk
    let chunk_1_proofs = {
        let mut zkevm_prover = prover::ChunkProver::from_params_and_assets(&params_map, ASSETS_DIR);
        log::info!("Constructed zkevm prover");
        chunk_1
            .into_iter()
            .map(|block_trace| {
                prove_and_verify_chunk(
                    &params_map,
                    &output_dir,
                    ChunkProvingTask::new(vec![block_trace]),
                    &mut zkevm_prover,
                    None,
                    false,
                )
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(chunk_1_proofs.len(), 1, "only 1 halo2-chunk");
    log::info!("halo2 chunk proof OK");

    // sp1 chunk
    let chunk_2_proofs = {
        let mut sp1_prover = prover::Sp1Prover::from_params_and_assets(&params_map, ASSETS_DIR);
        log::info!("Constructed sp1 prover");
        chunk_2
            .into_iter()
            .map(|block_trace| {
                prove_and_verify_sp1_chunk(
                    &params_map,
                    &output_dir,
                    ChunkProvingTask::new(vec![block_trace]),
                    &mut sp1_prover,
                    None,
                )
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(chunk_2_proofs.len(), 1, "only 1 sp1-chunk");
    log::info!("sp1 chunk proof OK");

    // dump chunk proof's SNARK protocol.
    chunk_1_proofs[0]
        .dump(&output_dir, "halo2")
        .expect("should dump protocol OK");
    chunk_2_proofs[0]
        .dump(&output_dir, "sp1")
        .expect("should dump protocol OK");
    log::info!("dumped chunk protocols OK");

    // generate batch proving task
    let batch_proving_task = {
        let dummy_parent_batch_hash = H256([
            0xab, 0xac, 0xad, 0xae, 0xaf, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0,
        ]);
        let chunks = vec![
            chunk_1_proofs[0].inner.chunk_info().clone(),
            chunk_2_proofs[0].inner.chunk_info().clone(),
        ];
        let blob_bytes = get_blob_from_chunks(&chunks);
        let batch_header = BatchHeader::construct_from_chunks(
            4,
            123,
            l1_message_popped,
            l1_message_popped,
            dummy_parent_batch_hash,
            last_block_timestamp,
            &chunks,
            &blob_bytes,
        );
        BatchProvingTask {
            chunk_proofs: vec![chunk_1_proofs[0].clone(), chunk_2_proofs[0].clone()],
            batch_header,
            blob_bytes,
        }
    };
    log::info!("batch proving task OK");

    let mut batch_prover = new_batch_prover(&params_map, &output_dir);
    let batch_proof = prove_and_verify_batch(
        &params_map,
        &output_dir,
        &mut batch_prover,
        batch_proving_task,
    );
    log::info!("batch proof generated OK");

    let bundle_proving_task = prover::BundleProvingTask {
        batch_proofs: vec![batch_proof],
    };
    prove_and_verify_bundle(&output_dir, &mut batch_prover, bundle_proving_task);
}

#[cfg(feature = "prove_verify")]
#[test]
fn test_batch_bundle_verify() -> anyhow::Result<()> {
    use integration::{
        prove::{new_batch_prover, prove_and_verify_batch, prove_and_verify_bundle},
        test_util::read_dir,
    };
    use itertools::Itertools;
    use prover::{read_json_deep, BundleProvingTask, ProverError, BATCH_PROVER_DEGREES};

    let output_dir = init_env_and_log("batch_bundle_tests");

    let params_map = prover::Prover::load_params_map(
        PARAMS_DIR,
        &BATCH_PROVER_DEGREES.iter().copied().collect_vec(),
    );
    let batch_tasks_paths = read_dir("./tests/test_data/batch_tasks")?;
    let batch_tasks: Vec<BatchProvingTask> = batch_tasks_paths
        .iter()
        .map(read_json_deep)
        .collect::<Result<Vec<_>, ProverError>>()?;

    log::info!("num batch tasks = {}", batch_tasks.len());

    let mut prover = new_batch_prover(&params_map, &output_dir);
    let batch_proofs = batch_tasks
        .into_iter()
        .map(|batch_task| prove_and_verify_batch(&params_map, &output_dir, &mut prover, batch_task))
        .collect::<Vec<_>>();

    assert_eq!(batch_proofs.len(), 10, "expecting 10 batches");

    log::info!("bundle 1 batch");
    prove_and_verify_bundle(
        &output_dir,
        &mut prover,
        BundleProvingTask {
            batch_proofs: batch_proofs[0..1].to_vec(),
        },
    );
    log::info!("bundle 1 batches OK");

    log::info!("bundle 2 batches");
    prove_and_verify_bundle(
        &output_dir,
        &mut prover,
        BundleProvingTask {
            batch_proofs: batch_proofs[1..3].to_vec(),
        },
    );
    log::info!("bundle 2 batches OK");

    log::info!("bundle 3 batches");
    prove_and_verify_bundle(
        &output_dir,
        &mut prover,
        BundleProvingTask {
            batch_proofs: batch_proofs[3..6].to_vec(),
        },
    );
    log::info!("bundle 3 batches OK");

    log::info!("bundle 4 batches");
    prove_and_verify_bundle(
        &output_dir,
        &mut prover,
        BundleProvingTask {
            batch_proofs: batch_proofs[6..10].to_vec(),
        },
    );
    log::info!("bundle 4 batches OK");

    Ok(())
}

fn gen_batch_proving_task(
    params_map: &BTreeMap<u32, ParamsKZG<Bn256>>,
    output_dir: &str,
    chunk_dirs: &[String],
    opt_batch_header: Option<BatchHeader<MAX_AGG_SNARKS>>,
) -> (BatchProvingTask, BatchHeader<MAX_AGG_SNARKS>) {
    let chunks: Vec<_> = chunk_dirs
        .iter()
        .map(|chunk_dir| load_chunk(chunk_dir).1)
        .collect();
    let l1_message_popped = chunks
        .iter()
        .flatten()
        .map(|chunk| chunk.num_l1_txs())
        .sum();
    let last_block_timestamp = chunks.last().map_or(0, |block_traces| {
        block_traces
            .last()
            .map_or(0, |block_trace| block_trace.header.timestamp.as_u64())
    });


    let chunk_proofs = if read_env_var("TEST_SP1", false) {
        let mut sp1_prover = Sp1Prover::from_params_and_assets(params_map, ASSETS_DIR);
        log::info!("Constructed sp1 prover");
        chunks
            .into_iter()
            .map(|block_traces| {
                prove_and_verify_sp1_chunk(
                    params_map,
                    output_dir,
                    ChunkProvingTask::new(block_traces),
                    &mut sp1_prover,
                    None,
                )
            })
            .collect::<Vec<_>>()
    } else {
        let mut zkevm_prover = ChunkProver::from_params_and_assets(params_map, ASSETS_DIR);
        log::info!("Constructed zkevm prover");
        chunks
            .into_iter()
            .map(|block_traces| {
                prove_and_verify_chunk(
                    params_map,
                    output_dir,
                    ChunkProvingTask::new(block_traces),
                    &mut zkevm_prover,
                    None,
                    false,
                )
            })
            .collect::<Vec<_>>()
    };

    log::info!("Generated chunk proofs");

    // dummy parent batch hash
    let dummy_parent_batch_hash = H256([
        0xab, 0xac, 0xad, 0xae, 0xaf, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0,
    ]);

    let chunks = chunk_proofs
        .clone()
        .into_iter()
        .map(|cp| cp.inner.chunk_info().clone())
        .collect::<Vec<_>>();

    let blob_bytes = get_blob_from_chunks(&chunks);

    let batch_header = BatchHeader::construct_from_chunks(
        opt_batch_header.map_or(4, |header| header.version),
        opt_batch_header.map_or(123, |header| header.batch_index + 1),
        l1_message_popped,
        opt_batch_header.map_or(l1_message_popped, |header| {
            header.total_l1_message_popped + l1_message_popped
        }),
        opt_batch_header.map_or(dummy_parent_batch_hash, |header| header.batch_hash()),
        last_block_timestamp,
        &chunks,
        &blob_bytes,
    );

    (
        BatchProvingTask {
            chunk_proofs,
            batch_header,
            blob_bytes,
        },
        batch_header,
    )
}

/*
#[cfg(feature = "fix_later")]
fn log_batch_pi(trace_paths: &[String]) {
    let max_num_snarks = prover::MAX_AGG_SNARKS;
    let chunk_traces: Vec<_> = trace_paths
        .iter()
        .map(|trace_path| {
            env::set_var("TRACE_PATH", trace_path);
            load_chunk_for_test().1
        })
        .collect();
    let l1_message_popped = chunk_traces
        .iter()
        .flatten()
        .map(|chunk| chunk.num_l1_txs())
        .sum();
    let last_block_timestamp = chunk_traces.last().map_or(0, |block_traces| {
        block_traces
            .last()
            .map_or(0, |block_trace| block_trace.header.timestamp.as_u64())
    });

    let mut chunk_hashes: Vec<ChunkInfo> = chunk_traces
        .into_iter()
        .map(|chunk_trace| {
            let witness_block = chunk_trace_to_witness_block(chunk_trace.clone()).unwrap();
            ChunkInfo::from_witness_block(&witness_block, false)
        })
        .collect();
    let blob_bytes = get_blob_from_chunks(&chunk_hashes);
    let real_chunk_count = chunk_hashes.len();
    if real_chunk_count < max_num_snarks {
        let mut padding_chunk_hash = chunk_hashes.last().unwrap().clone();
        padding_chunk_hash.is_padding = true;

        // Extend to MAX_AGG_SNARKS for both chunk hashes and layer-2 snarks.
        chunk_hashes
            .extend(std::iter::repeat(padding_chunk_hash).take(max_num_snarks - real_chunk_count));
    }

    // dummy parent batch hash
    let parent_batch_hash = H256([
        0xab, 0xac, 0xad, 0xae, 0xaf, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0,
    ]);
    let batch_header = BatchHeader {
        version: 3,
        batch_index: 123,
        l1_message_popped,
        total_l1_message_popped: l1_message_popped,
        parent_batch_hash,
        last_block_timestamp,
        ..Default::default() // these will be populated later.
    };
    let batch_hash = BatchHash::<{ prover::MAX_AGG_SNARKS }>::construct(
        &chunk_hashes,
        batch_header,
        &blob_bytes,
    );
    let blob = batch_hash.point_evaluation_assignments();

    let challenge = blob.challenge;
    let evaluation = blob.evaluation;
    println!("blob.challenge: {challenge:x}");
    println!("blob.evaluation: {evaluation:x}");
    for (i, elem) in blob.coefficients.iter().enumerate() {
        println!("blob.coeffs[{}]: {elem:x}", i);
    }
}
*/

fn dump_chunk_protocol(batch: &BatchProvingTask, output_dir: &str) {
    // Dump chunk-procotol to "protocol_chunk_{halo2,sp1}.protocol" for batch proving.
    let protocol = batch.chunk_proofs.first().unwrap();

    protocol.dump(output_dir, "halo2").unwrap();
    // adhoc!
    protocol.dump(output_dir, "sp1").unwrap();
}
