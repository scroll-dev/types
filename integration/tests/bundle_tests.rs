use integration::prove::{new_batch_prover, prove_and_verify_bundle};
use prover::{init_env_and_log, read_json, BatchProofV2, BundleProvingTask};

#[cfg(feature = "prove_verify")]
#[test]
fn test_bundle_prove_verify() {
    use integration::test_util::PARAMS_DIR;
    use itertools::Itertools;
    use prover::BATCH_PROVER_DEGREES;

    let output_dir = init_env_and_log("bundle_tests");
    log::info!("Initialized ENV and created output-dir {output_dir}");

    let params_map = prover::Prover::load_params_map(
        PARAMS_DIR,
        &BATCH_PROVER_DEGREES.iter().copied().collect_vec(),
    );

    let bundle_task = gen_bundle_proving_task(&[
        "tests/test_data/full_proof_batch_agg_1.json",
        "tests/test_data/full_proof_batch_agg_2.json",
    ]);
    // dump_chunk_protocol(&batch, &output_dir);
    let mut batch_prover = new_batch_prover(&params_map, "tests/test_data");
    prove_and_verify_bundle(&output_dir, &mut batch_prover, bundle_task);
}

fn gen_bundle_proving_task(batch_proof_files: &[&str]) -> BundleProvingTask {
    let mut batch_proofs = Vec::new();

    for proof_file in batch_proof_files {
        let batch_proof: BatchProofV2 = read_json(proof_file).unwrap();
        log::debug!(
            "Loaded batch-proofs, header {:#?}",
            batch_proof.inner.batch_hash
        );
        batch_proofs.push(batch_proof);
    }

    BundleProvingTask { batch_proofs }
}

#[ignore]
#[test]
fn test_bundle_prove_verify_after_batch() {
    use glob::glob;
    use integration::test_util::PARAMS_DIR;
    use itertools::Itertools;
    use prover::{read_json_deep, BatchProvingTask, BATCH_PROVER_DEGREES};

    let output_dir = init_env_and_log("bundle_tests");

    let mut batch_tasks = glob(&format!("{output_dir}/full_proof_batch_prove_?.json"))
        .unwrap()
        .map(|task_path| {
            read_json_deep::<_, BatchProvingTask>(task_path.unwrap().to_str().unwrap()).unwrap()
        })
        .collect::<Vec<_>>();

    batch_tasks
        .as_mut_slice()
        .sort_by_key(|task| task.batch_header.batch_index);

    let batch_proofs: Vec<BatchProofV2> = batch_tasks
        .iter()
        .map(|task| {
            log::info!("local batch proof {}", task.identifier());
            read_json_deep(&format!(
                "{output_dir}/full_proof_batch_{}.json",
                task.identifier()
            ))
            .unwrap()
        })
        .collect();

    let bundle = BundleProvingTask { batch_proofs };
    let params_map = prover::Prover::load_params_map(
        PARAMS_DIR,
        &BATCH_PROVER_DEGREES.iter().copied().collect_vec(),
    );

    let mut prover = new_batch_prover(&params_map, &output_dir);
    prove_and_verify_bundle(&output_dir, &mut prover, bundle);
}
