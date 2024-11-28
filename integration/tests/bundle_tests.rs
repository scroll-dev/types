use integration::prove::{new_batch_prover, prove_and_verify_bundle};
use prover::{init_env_and_log, read_json, BatchProofV2, BundleProvingTask};
//use std::{fs, path::PathBuf};

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


#[test]
fn test_evm_verifier_from_layer5() {
    use prover::{
        io::load_snark,
        config::LayerId,
        utils::read_env_var, 
    };
    use integration::test_util::PARAMS_DIR;
    use itertools::Itertools;
    use prover::config::AGG_DEGREES;

    let output_dir = init_env_and_log("test_evm_verifer");

    let layer5_snark_path = read_env_var(
        "LAYER5_SNARK", 
        "tests/test_data/recursion_snark_layer5.json".to_string());
    let snark = load_snark(&layer5_snark_path).ok().flatten().unwrap();

    let params_map = prover::common::Prover::load_params_map(
        PARAMS_DIR,
        &AGG_DEGREES.iter().copied().collect_vec(),
    );

    let mut evm_prover = prover::common::Prover::from_params_map(&params_map);

    // enforce general yul
    std::env::set_var("SCROLL_PROVER_DUMP_YUL", "true");
    evm_prover.load_or_gen_comp_evm_proof(
        "0",
        LayerId::Layer6.id(),
        true,
        LayerId::Layer6.degree(),
        snark,
        Some(output_dir.as_str()),
    ).unwrap();

}

#[ignore]
#[test]
fn test_evm_verifier_from_bin() {
    use prover::io::read_all;
    use revm::{
        primitives::{CreateScheme, ExecutionResult, Output, TransactTo, TxEnv},
        InMemoryDB, EVM,
    };

    let output_dir = init_env_and_log("test_evm_verifer");

    let bytecode = read_all(&format!("{output_dir}/evm_verifier.bin"));
    log::info!("bytecode len {}", bytecode.len());

    let mut evm = EVM {
        env: Default::default(),
        db: Some(InMemoryDB::default()),
    };

    // only deployment code
    evm.env.tx = TxEnv {
        gas_limit: u64::MAX,
        transact_to: TransactTo::Create(CreateScheme::Create),
        data: bytecode.into(),
        ..Default::default()
    };

    let result = evm.transact_commit().unwrap();
    let contract = match result {
        ExecutionResult::Success {
            output: Output::Create(_, Some(contract)),
            ..
        } => contract,
        ExecutionResult::Revert { gas_used, output } => {
            panic!(
                "Contract deployment transaction reverts with gas_used {gas_used} and output {:#x}",
                output
            )
        }
        ExecutionResult::Halt { reason, gas_used } => panic!(
                "Contract deployment transaction halts unexpectedly with gas_used {gas_used} and reason {:?}",
                reason
            ),
        _ => unreachable!(),
    };

    log::info!("contrace done at {}", contract);
}