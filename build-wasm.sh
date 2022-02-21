#!/bin/sh

BRANCH=$(git rev-parse --abbrev-ref HEAD)
if [ $BRANCH == "main" ]; then
	FEATURES="--features mainnet"
else
	FEATURES=""
fi

if [ $1 ]; then
	TARGET=$1
else
	TARGET="web"
fi

echo $PWD

cargo install agsol-glue --version 0.1.2-alpha.1
agsol-glue schema contract
RUST_LOG=debug agsol-glue wasm client --target $TARGET $FEATURES
