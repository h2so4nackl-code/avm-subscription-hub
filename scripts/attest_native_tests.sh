#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
out="${1:?Supply a new absolute output directory}"
[[ "$out" = /* ]] || { echo "Use an absolute output directory" >&2; exit 1; }
mode="${2:-demo}"
case "$mode" in
  demo|prepare|trusted) ;;
  *) echo "Expected demo, prepare, or trusted mode" >&2; exit 1 ;;
esac
work="$(mktemp -d "${RUNNER_TEMP:-/tmp}/avm-attest.XXXXXXXX")"
trap 'rm -rf "$work"' EXIT
cd "$root"

commit="$(git rev-parse HEAD)"
[[ -z "${GITHUB_SHA:-}" || "$commit" == "$GITHUB_SHA" ]] || {
  echo "Checkout differs from the dispatched commit" >&2
  exit 1
}
git diff --quiet HEAD -- || { echo "Tracked checkout has local changes" >&2; exit 1; }

if [[ "$mode" != trusted ]]; then
  mkdir "$out"
  # Invoke Cargo outside this repository so its vendored Cargo source policy
  # governs the subject tests and the CLI uses its own crates.io lockfile.
  (
    cd "$work"
    cargo +1.99.1 build --locked --manifest-path "$root/tools/avm-attest/Cargo.toml" \
      --target-dir "$work/target" --package avm-attest
  )
  install -m 700 "$work/target/debug/avm-attest" "$out/.avm-attest"
  bash scripts/restore_vendor.sh

  test_status=0
  CARGO_TARGET_DIR="$work/subject-target" cargo +1.93.0 test --workspace --locked --offline \
    -- --test-threads=1 > "$out/native-tests.txt" 2>&1 || test_status=$?
  tail -n 20 "$out/native-tests.txt"
  printf '%s\n' "$test_status" > "$out/test-exit-code.txt"
  printf '%s\n' "$commit" > "$out/.tested-commit"
fi
if [[ "$mode" == prepare ]]; then
  exit 0
fi

[[ "$(cat "$out/.tested-commit")" == "$commit" ]] || {
  echo "Prepared report belongs to a different commit" >&2
  exit 1
}
cli="$out/.avm-attest"
seed="$work/signer.seed"
if [[ "$mode" == trusted ]]; then
  if [[ ! "${ATTEST_SIGNING_SEED_B64:-}" =~ ^[A-Za-z0-9+/]{43}=$ ]] ||
     [[ ! "${ATTEST_PUBLIC_KEY:-}" =~ ^[a-f0-9]{64}$ ]]; then
    echo "Trusted mode requires a 32-byte Base64 seed and a lowercase hex public key" >&2
    exit 1
  fi
  (umask 077; printf '%s' "$ATTEST_SIGNING_SEED_B64" | base64 --decode > "$seed")
  unset ATTEST_SIGNING_SEED_B64
  printf '%s\n' "$ATTEST_PUBLIC_KEY" > "$out/signer.pub"
  signer_mode=trusted-persistent
else
  "$cli" keygen "$seed" > "$out/signer.pub"
  signer_mode=demo-ephemeral
fi

test_status="$(cat "$out/test-exit-code.txt")"
outcome=pass
if (( test_status != 0 )); then
  outcome=fail
fi

sdk_release="$(python3 -c 'import json; print(json.load(open("evidence/qualification.json"))["sdk_release"])')"
repository="${GITHUB_REPOSITORY:-h2so4nackl-code/avm-subscription-hub}"
"$cli" sign "$seed" "${repository,,}" "$commit" "$sdk_release" "$outcome" \
  "$out/native-tests.txt" > "$work/attestation.json"
rm -f "$seed"
"$cli" verify "$out/signer.pub" "$work/attestation.json" "$out/native-tests.txt" \
  > "$work/verified.json"

cp "$out/native-tests.txt" "$work/tampered.txt"
printf '\nChanged after signing\n' >> "$work/tampered.txt"
if "$cli" verify "$out/signer.pub" "$work/attestation.json" "$work/tampered.txt" \
    > "$work/tampered.out" 2> "$work/tampered.err"; then
  echo "Modified report unexpectedly passed verification" >&2
  exit 1
fi
grep -Fx 'avm-attest: report SHA-256 mismatch' "$work/tampered.err" \
  > "$out/tamper-check.txt"
mv "$work/attestation.json" "$work/verified.json" "$out/"
printf '%s\n' "$signer_mode" > "$out/signer-mode.txt"
rm -f "$out/.avm-attest" "$out/.tested-commit"
