use super::{mpt, MAX_CALLDATA, MAX_RWS, MAX_TXS};
use crate::circuit::{TargetCircuit, AUTO_TRUNCATE, DEGREE, MAX_INNER_BLOCKS, MAX_KECCAK_ROWS};
use bus_mapping::circuit_input_builder::{self, BlockHead, CircuitInputBuilder, CircuitsParams};
use bus_mapping::state_db::{Account, CodeDB, CodeHash, StateDB};
use eth_types::evm_types::OpcodeId;
use eth_types::{Hash, ToAddress};
use ethers_core::types::{Address, Bytes, U256};
use types::eth::{BlockTrace, EthBlock, ExecStep};

use mpt_zktrie::hash::Hashable;
use mpt_zktrie::state::ZktrieState;
use zkevm_circuits::evm_circuit::witness::block_apply_mpt_state;
use zkevm_circuits::evm_circuit::witness::{block_convert, Block};
use zkevm_circuits::util::SubCircuit;

use zkevm_circuits::bytecode_circuit::bytecode_unroller::HASHBLOCK_BYTES_IN_FIELD;

use halo2_proofs::arithmetic::FieldExt;
use halo2_proofs::halo2curves::bn256::Fr;

use anyhow::bail;
use is_even::IsEven;
use itertools::Itertools;
use std::collections::HashMap;
use std::time::Instant;

fn verify_proof_leaf<T: Default>(inp: mpt::TrieProof<T>, key_buf: &[u8; 32]) -> mpt::TrieProof<T> {
    let first_16bytes: [u8; 16] = key_buf[..16].try_into().expect("expect first 16 bytes");
    let last_16bytes: [u8; 16] = key_buf[16..].try_into().expect("expect last 16 bytes");

    let bt_high = Fr::from_u128(u128::from_be_bytes(first_16bytes));
    let bt_low = Fr::from_u128(u128::from_be_bytes(last_16bytes));

    if let Some(key) = inp.key {
        let rev_key_bytes: Vec<u8> = key.to_fixed_bytes().into_iter().rev().collect();
        let key_fr = Fr::from_bytes(&rev_key_bytes.try_into().unwrap()).unwrap();

        let secure_hash = Fr::hash([bt_high, bt_low]);

        if key_fr == secure_hash {
            inp
        } else {
            Default::default()
        }
    } else {
        inp
    }
}

fn extend_address_to_h256(src: &Address) -> [u8; 32] {
    let mut bts: Vec<u8> = src.as_bytes().into();
    bts.resize(32, 0);
    bts.as_slice().try_into().expect("32 bytes")
}

const SUB_CIRCUIT_NAMES: [&str; 10] = [
    "evm", "state", "bytecode", "copy", "keccak", "tx", "rlp", "exp", "pi", "mpt",
];

// TODO: optimize it later
pub fn calculate_row_usage_of_trace(block_trace: &BlockTrace) -> Result<Vec<usize>, anyhow::Error> {
    let witness_block = block_traces_to_witness_block(std::slice::from_ref(block_trace))?;
    let rows =
        <crate::circuit::SuperCircuit as TargetCircuit>::Inner::min_num_rows_block_subcircuits(
            &witness_block,
        )
        .0;

    log::debug!(
        "row usage of block {:?}, tx num {:?}, tx len sum {}, rows needed {:?}",
        block_trace.header.number,
        witness_block.txs.len(),
        witness_block
            .txs
            .iter()
            .map(|t| t.call_data_length)
            .sum::<usize>(),
        SUB_CIRCUIT_NAMES.iter().zip_eq(rows.iter())
    );
    Ok(rows)
}

/// ...
pub fn check_batch_capacity(block_traces: &mut Vec<BlockTrace>) -> Result<(), anyhow::Error> {
    let total_tx_count = block_traces
        .iter()
        .map(|b| b.transactions.len())
        .sum::<usize>();
    let total_tx_len_sum = block_traces
        .iter()
        .flat_map(|b| b.transactions.iter().map(|t| t.data.len()))
        .sum::<usize>();
    log::info!(
        "check capacity of block traces, block count {}, tx total num {}, tx total len {}",
        block_traces.len(),
        total_tx_count,
        total_tx_len_sum
    );

    if !*AUTO_TRUNCATE {
        log::debug!("AUTO_TRUNCATE=false, keep batch as is");
        return Ok(());
    }

    let t = Instant::now();
    let mut acc = Vec::new();
    let mut block_num = block_traces.len();
    for (idx, block) in block_traces.iter().enumerate() {
        let usage = calculate_row_usage_of_trace(block)?;
        if acc.is_empty() {
            acc = usage;
        } else {
            acc.iter_mut().zip(usage.iter()).for_each(|(acc, usage)| {
                *acc += usage;
            });
        }
        let rows = itertools::max(&acc).unwrap();
        let rows_and_names: Vec<(_, _)> = SUB_CIRCUIT_NAMES
            .iter()
            .zip_eq(acc.iter())
            .collect::<Vec<(_, _)>>();
        log::debug!(
            "row usage after block {}({:?}): {}, {:?}",
            idx,
            block.header.number,
            rows,
            rows_and_names
        );
        if *rows >= (1 << *DEGREE) - 256 {
            log::warn!("truncate blocks [{}..{})", idx, block_traces.len());
            block_num = idx;
            break;
        }
    }
    log::debug!("check_batch_capacity takes {:?}", t.elapsed());
    block_traces.truncate(block_num);
    let total_tx_count2 = block_traces
        .iter()
        .map(|b| b.transactions.len())
        .sum::<usize>();
    if total_tx_count != 0 && total_tx_count2 == 0 {
        // the circuit cannot even prove the first non-empty block...
        bail!("ciruit capacity not enough");
    }
    Ok(())
}

pub fn block_traces_to_witness_block(
    block_traces: &[BlockTrace],
) -> Result<Block<Fr>, anyhow::Error> {
    let old_root = if block_traces.is_empty() {
        eth_types::Hash::zero()
    } else {
        block_traces[0].storage_trace.root_before
    };
    let zktrie_state = ZktrieState::from_trace(
        old_root,
        block_traces.iter().rev().flat_map(|block| {
            block.storage_trace.proofs.iter().flat_map(|kv_map| {
                kv_map
                    .iter()
                    .map(|(k, bts)| (k, bts.iter().map(Bytes::as_ref)))
            })
        }),
        block_traces.iter().rev().flat_map(|block| {
            block
                .storage_trace
                .storage_proofs
                .iter()
                .flat_map(|(k, kv_map)| {
                    kv_map
                        .iter()
                        .map(move |(sk, bts)| (k, sk, bts.iter().map(Bytes::as_ref)))
                })
        }),
    )?;

    let chain_ids = block_traces
        .iter()
        .flat_map(|block_trace| block_trace.transactions.iter().map(|tx| tx.chain_id))
        .collect::<Vec<U256>>();

    let chain_id = if !chain_ids.is_empty() {
        chain_ids[0]
    } else {
        0i16.into()
    };

    let mut state_db = zktrie_state.state().clone();
    let (zero_coinbase_exist, _) = state_db.get_account(&Default::default());
    if !zero_coinbase_exist {
        state_db.set_account(
            &Default::default(),
            Account {
                nonce: Default::default(),
                balance: Default::default(),
                storage: HashMap::new(),
                // FIXME: 0 or keccak(nil)?
                code_hash: Default::default(),
            },
        );
    }

    let (_state_db_legacy, code_db) = build_statedb_and_codedb(block_traces)?;
    let circuit_params = CircuitsParams {
        max_rws: MAX_RWS,
        max_copy_rows: MAX_RWS,
        max_txs: MAX_TXS,
        max_calldata: MAX_CALLDATA,
        max_bytecode: MAX_CALLDATA,
        max_inner_blocks: MAX_INNER_BLOCKS,
        keccak_padding: Some(MAX_KECCAK_ROWS),
        max_exp_steps: 256,
    };
    let mut builder_block = circuit_input_builder::Block::from_headers(&[], circuit_params);
    builder_block.prev_state_root = U256::from(zktrie_state.root());
    let mut builder = CircuitInputBuilder::new(state_db.clone(), code_db, &builder_block);
    for (idx, block_trace) in block_traces.iter().enumerate() {
        let is_last = idx == block_traces.len() - 1;
        let eth_block: EthBlock = block_trace.clone().into();

        let mut geth_trace = Vec::new();
        for result in &block_trace.execution_results {
            geth_trace.push(result.into());
        }
        // TODO: Get the history_hashes.
        let mut header = BlockHead::new(chain_id, Vec::new(), &eth_block)?;
        // override zeroed minder field with additional "coinbase" field in blocktrace
        if let Some(address) = block_trace.coinbase.address {
            header.coinbase = address;
        }
        builder.block.headers.insert(header.number.as_u64(), header);
        builder.handle_block_inner(&eth_block, geth_trace.as_slice(), false, is_last)?;

        let per_block_metric = false;
        if per_block_metric {
            let t = Instant::now();
            let block = block_convert::<Fr>(&builder.block, &builder.code_db)?;
            log::debug!("block convert time {:?}", t.elapsed());
            let rows =
                <crate::circuit::SuperCircuit as TargetCircuit>::Inner::min_num_rows_block(&block);
            log::debug!(
                "after block {}, tx num {:?}, tx len sum {}, rows needed {:?}. estimate time: {:?}",
                idx,
                builder.block.txs().len(),
                builder
                    .block
                    .txs()
                    .iter()
                    .map(|t| t.input.len())
                    .sum::<usize>(),
                rows,
                t.elapsed()
            );
        }
    }
    builder.set_value_ops_call_context_rwc_eor();
    builder.set_end_block()?;

    let mut witness_block = block_convert(&builder.block, &builder.code_db)?;
    witness_block.evm_circuit_pad_to = MAX_RWS;
    log::debug!(
        "witness_block.circuits_params {:?}",
        witness_block.circuits_params
    );

    block_apply_mpt_state(&mut witness_block, zktrie_state);
    Ok(witness_block)
}

pub fn decode_bytecode(bytecode: &str) -> Result<Vec<u8>, anyhow::Error> {
    let mut stripped = if let Some(stripped) = bytecode.strip_prefix("0x") {
        stripped.to_string()
    } else {
        bytecode.to_string()
    };

    let bytecode_len = stripped.len() as u64;
    if !bytecode_len.is_even() {
        stripped = format!("0{stripped}");
    }

    hex::decode(stripped).map_err(|e| e.into())
}

#[derive(Debug, Clone)]
struct PoseidonCodeHash {
    bytes_in_field: usize,
}

impl PoseidonCodeHash {
    fn new(bytes_in_field: usize) -> Self {
        Self { bytes_in_field }
    }
}

impl CodeHash for PoseidonCodeHash {
    fn hash_code(&self, code: &[u8]) -> Hash {
        use halo2_proofs::halo2curves::group::ff::PrimeField;
        use mpt_zktrie::hash::MessageHashable;
        let fls = (0..(code.len() / self.bytes_in_field))
            .map(|i| i * self.bytes_in_field)
            .map(|i| {
                let mut buf: [u8; 32] = [0; 32];
                U256::from_big_endian(&code[i..i + self.bytes_in_field]).to_little_endian(&mut buf);
                Fr::from_bytes(&buf).unwrap()
            });
        let msgs: Vec<_> = fls
            .chain(if code.len() % self.bytes_in_field == 0 {
                None
            } else {
                let last_code = &code[code.len() - code.len() % self.bytes_in_field..];
                // pad to bytes_in_field
                let mut last_buf = vec![0u8; self.bytes_in_field];
                last_buf.as_mut_slice()[..last_code.len()].copy_from_slice(last_code);
                let mut buf: [u8; 32] = [0; 32];
                U256::from_big_endian(&last_buf).to_little_endian(&mut buf);
                Some(Fr::from_bytes(&buf).unwrap())
            })
            .collect();

        let h = Fr::hash_msg(&msgs, Some(code.len() as u64));

        let mut buf: [u8; 32] = [0; 32];
        U256::from_little_endian(h.to_repr().as_ref()).to_big_endian(&mut buf);
        Hash::from_slice(&buf)
    }
}

#[test]
fn code_hashing() {
    let code_hasher = PoseidonCodeHash::new(16);
    let simple_byte: [u8; 1] = [0];
    assert_eq!(
        format!("{:?}", code_hasher.hash_code(&simple_byte)),
        "0x0ee069e6aa796ef0e46cbd51d10468393d443a00f5affe72898d9ab62e335e16"
    );

    let simple_byte: [u8; 2] = [0, 1];
    assert_eq!(
        format!("{:?}", code_hasher.hash_code(&simple_byte)),
        "0x26cd650aa0d0b9aada79f5f7c03c5961430c12a2142832789fc31a4188d762ff"
    );

    let example = "608060405234801561001057600080fd5b506004361061004c5760003560e01c806321848c46146100515780632e64cec11461006d578063b0f2b72a1461008b578063f3417673146100a7575b600080fd5b61006b60048036038101906100669190610116565b6100c5565b005b6100756100da565b604051610082919061014e565b60405180910390f35b6100a560048036038101906100a09190610116565b6100e3565b005b6100af6100ed565b6040516100bc919061014e565b60405180910390f35b8060008190555060006100d757600080fd5b50565b60008054905090565b8060008190555050565b6000806100f957600080fd5b600054905090565b60008135905061011081610173565b92915050565b60006020828403121561012857600080fd5b600061013684828501610101565b91505092915050565b61014881610169565b82525050565b6000602082019050610163600083018461013f565b92915050565b6000819050919050565b61017c81610169565b811461018757600080fd5b5056fea2646970667358221220f4bca934426c76c7cb87cc32876fc6e65d1d7de23424faa61c347ffed95c449064736f6c63430008040033";
    let bytes = hex::decode(example).unwrap();

    assert_eq!(
        format!("{:?}", code_hasher.hash_code(&bytes)),
        "0x0e6d089fa72b508b90e014b486d64a5311df3030c45b10a95366cf53cd1ec9d5"
    );
}

/*
fn get_account_deployed_codehash(
    execution_result: &ExecutionResult,
) -> Result<eth_types::H256, anyhow::Error> {
    let created_acc = execution_result
        .account_created
        .as_ref()
        .expect("called when field existed")
        .address
        .as_ref()
        .unwrap();
    for state in &execution_result.account_after {
        if Some(created_acc) == state.address.as_ref() {
            return state.code_hash.ok_or_else(|| anyhow!("empty code hash"));
        }
    }
    Err(anyhow!("can not find created address in account after"))
}
fn get_account_created_codehash(step: &ExecStep) -> Result<eth_types::H256, anyhow::Error> {
    let extra_data = step
        .extra_data
        .as_ref()
        .ok_or_else(|| anyhow!("no extra data in create context"))?;
    let proof_list = extra_data
        .proof_list
        .as_ref()
        .expect("should has proof list");
    if proof_list.len() < 2 {
        Err(anyhow!("wrong fields in create context"))
    } else {
        proof_list[1]
            .code_hash
            .ok_or_else(|| anyhow!("empty code hash in final state"))
    }
}
*/
fn trace_code(cdb: &mut CodeDB, step: &ExecStep, sdb: &StateDB, code: Bytes, stack_pos: usize) {
    let stack = step
        .stack
        .as_ref()
        .expect("should have stack in call context");
    let addr = stack[stack.len() - stack_pos - 1].to_address(); //stack N-stack_pos

    let hash = cdb.insert(code.to_vec());

    // sanity check
    let (existed, data) = sdb.get_account(&addr);
    if existed && !(data.nonce.is_zero() && data.balance.is_zero()) {
        assert_eq!(
            hash, data.code_hash,
            "invalid codehash for existed account {addr:?}, {data:?}"
        );
    };
}
pub fn build_statedb_and_codedb(blocks: &[BlockTrace]) -> Result<(StateDB, CodeDB), anyhow::Error> {
    let mut sdb = StateDB::new();
    let mut cdb =
        CodeDB::new_with_code_hasher(Box::new(PoseidonCodeHash::new(HASHBLOCK_BYTES_IN_FIELD)));

    // step1: insert proof into statedb
    for block in blocks.iter().rev() {
        let storage_trace = &block.storage_trace;
        if let Some(acc_proofs) = &storage_trace.proofs {
            for (addr, acc) in acc_proofs.iter() {
                let acc_proof: mpt::AccountProof = acc.as_slice().try_into()?;
                let acc = verify_proof_leaf(acc_proof, &extend_address_to_h256(addr));
                if acc.key.is_some() {
                    // a valid leaf
                    let (_, acc_mut) = sdb.get_account_mut(addr);
                    acc_mut.nonce = acc.data.nonce.into();
                    acc_mut.code_hash = acc.data.code_hash;
                    acc_mut.balance = acc.data.balance;
                } else {
                    // it is essential to set it as default (i.e. not existed account data)
                    sdb.set_account(
                        addr,
                        Account {
                            nonce: Default::default(),
                            balance: Default::default(),
                            storage: HashMap::new(),
                            code_hash: Default::default(),
                        },
                    );
                }
            }
        }

        for (addr, s_map) in storage_trace.storage_proofs.iter() {
            let (found, acc) = sdb.get_account_mut(addr);
            if !found {
                log::error!("missed address in proof field show in storage: {:?}", addr);
                continue;
            }

            for (k, val) in s_map {
                let mut k_buf: [u8; 32] = [0; 32];
                k.to_big_endian(&mut k_buf[..]);
                let val_proof: mpt::StorageProof = val.as_slice().try_into()?;
                let val = verify_proof_leaf(val_proof, &k_buf);

                if val.key.is_some() {
                    // a valid leaf
                    acc.storage.insert(*k, *val.data.as_ref());
                //                log::info!("set storage {:?} {:?} {:?}", addr, k, val.data);
                } else {
                    // add 0
                    acc.storage.insert(*k, Default::default());
                    //                log::info!("set empty storage {:?} {:?}", addr, k);
                }
            }
        }

        // step2: insert code into codedb
        // notice empty codehash always kept as keccak256(nil)
        cdb.insert(Vec::new());

        for execution_result in &block.execution_results {
            if let Some(bytecode) = &execution_result.byte_code {
                let hash = cdb.insert(decode_bytecode(bytecode)?.to_vec());

                if execution_result.account_created.is_none() {
                    assert_eq!(Some(hash), execution_result.code_hash);
                }
            }

            for step in execution_result.exec_steps.iter().rev() {
                if let Some(data) = &step.extra_data {
                    match step.op {
                        OpcodeId::CALL
                        | OpcodeId::CALLCODE
                        | OpcodeId::DELEGATECALL
                        | OpcodeId::STATICCALL => {
                            let callee_code = data.get_code_at(1);
                            trace_code(&mut cdb, step, &sdb, callee_code, 1);
                        }
                        OpcodeId::CREATE | OpcodeId::CREATE2 => {
                            // notice we do not need to insert code for CREATE,
                            // bustmapping do this job
                        }
                        OpcodeId::EXTCODESIZE | OpcodeId::EXTCODECOPY => {
                            let code = data.get_code_at(0);
                            trace_code(&mut cdb, step, &sdb, code, 0);
                        }

                        _ => {}
                    }
                }
            }
        }
    }

    // A temporary fix: zkgeth do not trace 0 address if it is only refered as coinbase
    // (For it is not the "real" coinbase address in PoA) but would still refer it for
    // other reasons (like being transferred or called), in the other way, busmapping
    // seems always refer it as coinbase (?)
    // here we just add it as unexisted account and consider fix it in zkgeth later (always
    // record 0 addr inside storageTrace field)
    let (zero_coinbase_exist, _) = sdb.get_account(&Default::default());
    if !zero_coinbase_exist {
        sdb.set_account(
            &Default::default(),
            Account {
                nonce: Default::default(),
                balance: Default::default(),
                storage: HashMap::new(),
                code_hash: Default::default(),
            },
        );
    }

    Ok((sdb, cdb))
}

/*
pub fn trace_proof(sdb: &mut StateDB, proof: Option<AccountProofWrapper>) {
    // `to` may be empty
    if proof.is_none() {
        return;
    }
    let proof = proof.unwrap();

    let (found, acc) = sdb.get_account(&proof.address.unwrap());
    let mut storage = match found {
        true => acc.storage.clone(),
        false => HashMap::new(),
    };

    if let Some(s) = &proof.storage {
        log::trace!(
            "trace_proof ({:?}, {:?}) => {:?}",
            &proof.address.unwrap(),
            s.key.unwrap(),
            s.value.unwrap()
        );
        storage.insert(s.key.unwrap(), s.value.unwrap());
    }

    sdb.set_account(
        &proof.address.unwrap(),
        Account {
            nonce: proof.nonce.unwrap().into(),
            balance: proof.balance.unwrap(),
            storage,
            code_hash: proof.code_hash.unwrap(),
        },
    )
}
*/
