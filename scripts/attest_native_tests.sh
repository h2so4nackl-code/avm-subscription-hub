#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
out="${1:?Supply a new absolute output directory}"
[[ "$out" = /* ]] || { echo "Use an absolute output directory" >&2; exit 1; }
mkdir "$out"
work="$(mktemp -d "${RUNNER_TEMP:-/tmp}/avm-attest.XXXXXXXX")"
trap 'rm -rf "$work"' EXIT
cd "$root"

commit="$(git rev-parse HEAD)"
[[ -z "${GITHUB_SHA:-}" || "$commit" == "$GITHUB_SHA" ]] || {
  echo "Checkout differs from the dispatched commit" >&2
  exit 1
}
git diff --quiet HEAD -- || { echo "Tracked checkout has local changes" >&2; exit 1; }

# Invoke Cargo outside this repository so its vendored Cargo source policy
# governs the subject tests and the CLI uses its own crates.io lockfile.
(
  cd "$work"
  cargo +1.99.1 build --locked --manifest-path "$root/tools/avm-attest/Cargo.toml" \
    --target-dir "$work/target" --package avm-attest
)
cli="$work/target/debug/avm-attest"
bash scripts/restore_vendor.sh

test_status=0
CARGO_TARGET_DIR="$work/subject-target" cargo +1.93.0 test --workspace --locked --offline \
  -- --test-threads=1 > "$out/native-tests.txt" 2>&1 || test_status=$?
tail -n 20 "$out/native-tests.txt"
outcome=pass
if (( test_status != 0 )); then
  outcome=fail
fi

sdk_release="$(python3 -c 'import json; print(json.load(open("evidence/qualification.json"))["sdk_release"])')"
repository="${GITHUB_REPOSITORY:-h2so4nackl-code/avm-subscription-hub}"
"$cli" keygen "$work/signer.seed" > "$out/signer.pub"
"$cli" sign "$work/signer.seed" "${repository,,}" "$commit" "$sdk_release" "$outcome" \
  "$out/native-tests.txt" > "$out/attestation.json"
"$cli" verify "$out/signer.pub" "$out/attestation.json" "$out/native-tests.txt" \
  > "$out/verified.json"

cp "$out/native-tests.txt" "$work/tampered.txt"
printf '\nChanged after signing\n' >> "$work/tampered.txt"
if "$cli" verify "$out/signer.pub" "$out/attestation.json" "$work/tampered.txt" \
    > "$work/tampered.out" 2> "$work/tampered.err"; then
  echo "Modified report unexpectedly passed verification" >&2
  exit 1
fi
grep -Fx 'avm-attest: report SHA-256 mismatch' "$work/tampered.err" \
  > "$out/tamper-check.txt"
printf 'demo-ephemeral\n' > "$out/signer-mode.txt"
printf '%s\n' "$test_status" > "$out/test-exit-code.txt"
