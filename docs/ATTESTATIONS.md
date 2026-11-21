# Trusted AVM build attestations

The **AVM trusted build attestation** workflow runs manually on `main` and signs
the native test report for the dispatched commit. The signing step reads the
persistent Ed25519 key from the `avm-attestation-signing` GitHub environment.
It verifies the signature against the configured public key before uploading
the report. The **AVM build attestation demo** uses a disposable key.

## Set up the signer

Create the environment under **Settings → Environments**. Under **Deployment
branches and tags**, select **Selected branches and tags**, then add a
**Branch** rule for `main`.

From a Linux or WSL machine with access to the repository and an authenticated
GitHub CLI, run the following from the repository root. Use a private key
directory on the Linux filesystem. Keep a secure backup of the seed for key
recovery.

```bash
(
set -euo pipefail
repo_root="$(pwd -P)"
signer_dir="$HOME/.local/share/avm-attest/subscription-hub"
install -d -m 700 "$signer_dir"
rustup toolchain install 1.99.1 --profile minimal
(
  cd "$signer_dir"
  cargo +1.99.1 build --locked --manifest-path "$repo_root/tools/avm-attest/Cargo.toml" \
    --target-dir "$signer_dir/target" --package avm-attest
)
public_key="$("$signer_dir/target/debug/avm-attest" keygen "$signer_dir/signer.seed")"
printf '%s\n' "$public_key" > "$signer_dir/signer.pub"
base64 -w0 "$signer_dir/signer.seed" |
  gh secret set AVM_ATTEST_SIGNING_SEED_B64 \
    --repo h2so4nackl-code/avm-subscription-hub --env avm-attestation-signing
gh variable set AVM_ATTEST_PUBLIC_KEY \
  --repo h2so4nackl-code/avm-subscription-hub --env avm-attestation-signing \
  --body "$public_key"
)
```

Publish or share `signer.pub` through a channel verifiers already trust, such as
a signed release. Keep that copy independent of the workflow artifact.

## Run and verify

```bash
gh workflow run trusted-attestation.yml \
  --repo h2so4nackl-code/avm-subscription-hub --ref main
```

Download the `avm-trusted-attestation-*` artifact from the run page. The signed
claim covers the commit, test outcome, SDK context and SHA-256 of
`native-tests.txt`. To verify, pass your previously trusted copy of
`signer.pub` to the CLI.

```bash
avm-attest verify /path/to/trusted/signer.pub attestation.json native-tests.txt
```
