use std::{
    io::Read,
    path::{Path, PathBuf},
};

use glob::glob;
use prover::{eth_types::l2_types::BlockTrace, get_block_trace_from_file, read_env_var};
use serde_json::from_value;

pub const ASSETS_DIR: &str = "./test_assets";
pub const PARAMS_DIR: &str = "./params";

pub fn read_all<P>(filename: P) -> Vec<u8>
where
    P: AsRef<Path>,
{
    let mut buf = vec![];
    let mut fd = std::fs::File::open(filename).unwrap();
    fd.read_to_end(&mut buf).unwrap();
    buf
}

pub fn trace_path_for_test() -> String {
    // use trace file of post-curie upgrade
    read_env_var(
        "TRACE_PATH",
        "tests/extra_traces/batch1/chunk_1/block_7156762.json".to_string(),
    )
}

pub fn load_chunk_for_test() -> (Vec<String>, Vec<BlockTrace>) {
    load_chunk(&trace_path_for_test())
}

fn get_block_trace_from_file_compatible_chunk<P: AsRef<Path>>(path: P) -> BlockTrace {
    let mut buffer = Vec::new();
    let mut f = std::fs::File::open(&path).unwrap();
    f.read_to_end(&mut buffer).unwrap();

    use serde::Deserialize;
    use serde_json::Value;
    #[derive(Deserialize, Default, Debug, Clone)]
    struct BlockTraceJsonRpcResult {
        pub result: Value,
    }

    let mut raw_val = serde_json::from_slice::<BlockTraceJsonRpcResult>(&buffer)
    .map(|r|r.result)
    .unwrap_or_else(|e1| {
        serde_json::from_slice::<Value>(&buffer)
            .map_err(|e2| {
                panic!(
                    "unable to load raw BlockTrace from {:?}, {:?}, {:?}",
                    path.as_ref(),
                    e1,
                    e2
                )
            })
            .unwrap()
    });

    // sanity check
    match &mut raw_val {
        Value::Object(m) => {
            if m.get("executionResults").is_none() {
                m.insert(
                    "executionResults".to_string(), 
                    Value::Array(Vec::new()));
            }
        },
        _ => panic!("unexpected json type, should be object")
    };

    from_value(raw_val).unwrap()

}

fn list_chunk_file(trace_path: &str) -> Vec<String> {
    let paths: Vec<String> = if !std::fs::metadata(trace_path).unwrap().is_dir() {
        vec![trace_path.to_string()]
    } else {
        // Nested dirs are not allowed
        let mut file_names: Vec<String> = glob(&format!("{trace_path}/*.json"))
            .unwrap()
            .map(|p| p.unwrap().to_str().unwrap().to_string())
            .collect();
        file_names.sort_by_key(|s| {
            let path = Path::new(s);
            let basename = path.file_stem().unwrap().to_str().unwrap();
            basename
                .trim_start_matches("block_")
                .parse::<u32>()
                .unwrap()
        });
        file_names
    };
    log::info!("test cases traces: {:?}", paths);
    paths
}

pub fn load_chunk(trace_path: &str) -> (Vec<String>, Vec<BlockTrace>) {
    let paths = list_chunk_file(trace_path);
    let traces: Vec<_> = paths.iter().map(get_block_trace_from_file).collect();
    (paths, traces)
}

pub fn load_chunk_compatible(trace_path: &str) -> (Vec<String>, Vec<BlockTrace>) {
    let paths = list_chunk_file(trace_path);
    let traces: Vec<_> = paths.iter().map(get_block_trace_from_file_compatible_chunk).collect();
    (paths, traces)
}

pub fn load_batch(batch_dir: &str) -> anyhow::Result<Vec<String>> {
    let sorted_dirs = read_dir_recursive(batch_dir, "chunk_")?;
    log::info!("batch content: {:?}", sorted_dirs);
    Ok(sorted_dirs)
}

/// Reads inside a directory recursively and returns paths to all sub-directories that match the
/// given prefix.
pub fn read_dir_recursive(dir: impl AsRef<Path>, prefix: &str) -> anyhow::Result<Vec<String>> {
    let mut sorted_dirs: Vec<String> = std::fs::read_dir(dir)?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .map(|path| path.to_string_lossy().into_owned())
        .collect::<Vec<String>>();
    sorted_dirs.sort_by_key(|s| {
        let path = Path::new(s);
        let dir_name = path.file_name().unwrap().to_string_lossy();
        dir_name.trim_start_matches(prefix).parse::<u32>().unwrap()
    });
    Ok(sorted_dirs)
}

/// Reads inside a directory and returns all files.
pub fn read_dir(dir: impl AsRef<Path>) -> anyhow::Result<Vec<PathBuf>> {
    let mut sorted_files = std::fs::read_dir(dir)?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.is_file())
        .collect::<Vec<_>>();
    sorted_files.sort();
    Ok(sorted_files)
}
