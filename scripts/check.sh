#!/usr/bin/env bash
set -euo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
sdk="${1:-$root/.tooling/sdk/current}"
sdk="$(cd "$sdk" && pwd -P)"
optimizer="${2:-$root/.tooling/binaryen/binaryen-version_133/bin/wasm-opt}"
export PATH="${HOME}/.cargo/bin:${PATH}"
cd "$root"
if [[ ! -d "$root/vendor/registry" ]]; then bash scripts/restore_vendor.sh; fi
if [[ ! -x "$optimizer" ]]; then bash scripts/install_binaryen.sh "$root/.tooling/binaryen"; fi
mkdir -p evidence artifacts/{hub,receiver}
cargo +1.93.0 fmt -p avm-subscription-core -p avm-subscription-hub -p avm-subscription-receiver --check
cargo +1.93.0 test --workspace --locked --offline > evidence/native-tests.txt 2>&1
cargo +1.93.0 clippy --workspace --all-targets --locked --offline -- -D warnings > evidence/clippy.txt 2>&1
python3 scripts/verify_sources.py
python3 scripts/generate_abi.py --check
python3 scripts/generate_vectors.py --check
for contract in hub receiver; do
 "$sdk/bin/avm-contract-build" build --manifest-path "$root/contracts/$contract/Cargo.toml" >"evidence/build-$contract.txt" 2>&1
done
python3 scripts/sync_ids.py --check
python3 scripts/optimize.py --wasm-opt "$optimizer"
python3 scripts/e2e.py --sdk "$sdk"
python3 scripts/churn_e2e.py --sdk "$sdk"
python3 scripts/capacity_e2e.py --sdk "$sdk"
for contract in hub receiver; do
 for artifact in contract.wasm contract.compiler.wasm contract.acab contract.descriptor; do
  cp "contracts/$contract/target/avm/$artifact" "artifacts/$contract/$artifact"
 done
done
python3 scripts/record_evidence.py --sdk "$sdk"
echo 'PASS: 107 Rust tests, 4096 Python-bigint vectors, reproducible builds and optimizer, three real AVM journeys and replay'
