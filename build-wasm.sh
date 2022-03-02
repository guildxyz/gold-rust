#!/bin/sh

if [ $1 == "main" ]; then
	WASM_FEATURES="--features mainnet"
	TARGET_BRANCH=$1
else
	WASM_FEATURES=""
	TARGET_BRANCH="dev"
fi

# if a second argument is present that means
# we would like to build wasm to a different target from "web"
if [ $2 ]; then
	WASM_TARGET=$2
	# target branch name + wasm target (e.g. dev-nodejs)
	TARGET_BRANCH="${TARGET_BRANCH}-${WASM_TARGET}"
else
	WASM_TARGET="web"
fi

echo "Target branch: ${TARGET_BRANCH}"
echo "Wasm build target: ${WASM_TARGET}"
echo "Wasm build features: ${WASM_FEATURES}"

wasm-pack build client --target ${WASM_TARGET} --out-name index --out-dir ../gold-wasm ${WASM_FEATURES}

cd gold-wasm
rm .gitignore
git init
git add -A
git commit -m "Auto-generated wasm code"
git remote add origin https://${ACCESS_HEADER}github.com/agoraxyz/gold-glue.git
git branch -M $TARGET_BRANCH
git push -uf origin $TARGET_BRANCH

cd ..
rm -rf gold-wasm
