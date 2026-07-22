CONTRACT_DIR := Stellar-contracts-v1

.PHONY: all clean build test deploy-testnet deploy-mainnet

all: build test

clean:
	cd $(CONTRACT_DIR) && cargo clean

build:
	cd $(CONTRACT_DIR) && cargo build --target wasm32-unknown-unknown --release

test:
	cd $(CONTRACT_DIR) && cargo test

deploy-testnet:
	bash scripts/deploy_testnet.sh

deploy-mainnet:
	bash scripts/deploy_mainnet.sh
