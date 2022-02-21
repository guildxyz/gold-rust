#!/bin/sh

BRANCH=$(git rev-parse --abbrev-ref HEAD)
if [[ $BRANCH == "main" ]]; then
	FEATURES="--features mainnet"
else
	FEATURES=""
fi

if [[ $1 ]]; then
	TARGET=$1
else
	TARGET="web"
fi

echo $FEATURES
echo $TARGET

cargo install agsol-glue
agsol-glue schema contract
agsol-glue wasm client --target $TARGET $FEATURES
