CURRENTDATE=`date +"%Y-%m-%d"`

CHAIN_ID ?= 534352
export CHAIN_ID
RUST_MIN_STACK ?= 16777216
export RUST_MIN_STACK
RUST_BACKTRACE=1
export RUST_BACKTRACE

help: ## Display this help screen
	@grep -h \
		-E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | \
		awk 'BEGIN {FS = ":.*?## "}; {printf "\033[36m%-30s\033[0m %s\n", $$1, $$2}'

test-ci: fmt clippy test ## Run all the CI checks locally (in your actual toolchain)

test-all: test-ci ## Run all available tests

build-release: ## Check build in release mode
	@cargo build --release

fmt: ## Check whether the code is formatted correctly
	@cargo fmt --all -- --check

clippy: ## Run clippy checks over all workspace members
	@cargo check --all-features
	@cargo clippy --all-features --all-targets -- -D warnings

test: ## Run tests for all the workspace members
	@cargo test --release -p integration --test unit_tests

chain-prover:
	@cargo run --bin chain_prover --release

test-mock-prove:
	@cargo test --release -p integration --test mock_tests test_mock_prove -- --exact --nocapture

test-inner-prove:
	@cargo test --release -p integration --test inner_tests test_inner_prove_verify -- --exact --nocapture

test-chunk-prove:
	@cargo test --release -p integration --test chunk_tests test_chunk_prove_verify -- --exact --nocapture

test-batch-prove:
	@SCROLL_PROVER_DUMP_YUL=true cargo test --release -p integration --test batch_tests test_batch_prove_verify -- --exact --nocapture

test-bundle-prove:
	@SCROLL_PROVER_DUMP_YUL=true cargo test --release -p integration --test bundle_tests test_bundle_prove_verify -- --exact --nocapture

test-e2e-prove:
	@SCROLL_PROVER_DUMP_YUL=true cargo test --release -p integration --test e2e_tests test_e2e_prove_verify -- --exact --nocapture

test-e2e-prove-hybrid:
	@SCROLL_PROVER_DUMP_YUL=true cargo test --release -p integration --test e2e_tests test_e2e_prove_verify_hybrid -- --exact --nocapture

test-batch-bundle-prove:
	@SCROLL_PROVER_DUMP_YUL=true cargo test --release -p integration --test e2e_tests test_batch_bundle_verify -- --exact --nocapture

test-ccc:
	@cargo test --release -p integration --test unit_tests test_capacity_checker -- --exact --nocapture

# Could be called as `make download-setup -e degree=DEGREE params_dir=PARAMS_DIR`.
# As default `degree=25` and `params_dir=./integration/params`.
download-setup:
	sh download_setup.sh ${degree} ${params_dir}

.PHONY: help fmt clippy test test-ci test-all
