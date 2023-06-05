use crate::utils::get_block_trace_from_file;
use crate::utils::read_env_var;
use glob::glob;
use types::eth::BlockTrace;

pub mod mock_plonk;

pub const PARAMS_DIR: &str = "./test_params";

pub fn load_batch_traces(batch_dir: &str) -> (Vec<String>, Vec<types::eth::BlockTrace>) {
    let file_names: Vec<String> = glob(&format!("{batch_dir}/**/*.json"))
        .unwrap()
        .map(|p| p.unwrap().to_str().unwrap().to_string())
        .collect();
    log::info!("test batch with {:?}", file_names);
    let mut names_and_traces = file_names
        .into_iter()
        .map(|trace_path| {
            let trace: BlockTrace = get_block_trace_from_file(trace_path.clone());
            (
                trace_path,
                trace.clone(),
                trace.header.number.unwrap().as_u64(),
            )
        })
        .collect::<Vec<_>>();
    names_and_traces.sort_by(|a, b| a.2.cmp(&b.2));
    log::info!(
        "sorted: {:?}",
        names_and_traces
            .iter()
            .map(|(f, _, _)| f.clone())
            .collect::<Vec<String>>()
    );
    names_and_traces.into_iter().map(|(f, t, _)| (f, t)).unzip()
}

pub fn parse_trace_path_from_mode(mode: &str) -> &'static str {
    let trace_path = match mode {
        "empty" => "./tests/traces/empty.json",
        "greeter" => "./tests/traces/greeter_setValue.json",
        "single" => "./tests/traces/erc20/erc20_1_transfer.json",
        "multiple" => "./tests/traces/erc20/erc20_10_transfer.json",
        "native" => "./tests/traces/native_transfer.json",
        "dao" => "./tests/traces/dao/dao_dao-Propose.json",
        "nft" => "./tests/traces/nft/nft_mint.json",
        "sushi" => "./tests/traces/sushi/sushi_chef-withdraw.json",
        _ => "./tests/traces/erc20/erc20_10_transfer.json",
    };
    log::info!("using mode {:?}, testing with {:?}", mode, trace_path);
    trace_path
}

pub fn load_block_traces_for_test() -> (Vec<String>, Vec<BlockTrace>) {
    let trace_path: String = read_env_var("TRACE_PATH", "".to_string());
    let paths: Vec<String> = if trace_path.is_empty() {
        // use mode
        let mode = read_env_var("MODE", "multiple".to_string());
        if mode.to_lowercase() == "batch" || mode.to_lowercase() == "pack" {
            (1..=22)
                .map(|i| format!("tests/traces/bridge/{i:02}.json"))
                .collect()
        } else {
            vec![parse_trace_path_from_mode(&mode).to_string()]
        }
    } else if !std::fs::metadata(&trace_path).unwrap().is_dir() {
        vec![trace_path]
    } else {
        load_batch_traces(&trace_path).0
    };
    log::info!("test cases traces: {:?}", paths);
    let traces: Vec<_> = paths.iter().map(get_block_trace_from_file).collect();
    (paths, traces)
}
