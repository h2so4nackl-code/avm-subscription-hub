# AVM Subscription Hub

A two-asset, multi-merchant prepaid subscription system in Rust for **AVM SDK
0.3.0**. Customers reserve a budget at an explicitly accepted price/version;
completed periods accrue to merchants, partial cancellation is prorated, and
actual ECC withdrawals settle through authenticated runtime callbacks.

The project has a shared `no_std` financial core, a deployable Hub and a companion
recipient contract. It is intended as a useful SDK test-suite example, with
bounded storage and explicit failure semantics. It is not an audited custody
service or a claim of mainnet compatibility.

## Useful behavior

- Two configured, distinct ECC assets; principal is isolated from SHELL fees/rent.
- Up to 4 merchants, 8 versioned plans, 8 customer wallets and 16 simultaneous
  subscriptions. Closed subscription slots are reusable; IDs are never reused.
- Customer consent includes the expected plan version. Later repricing or
  deactivation does not change existing subscription terms.
- Prepaid terms, extension at the accepted price and permissionless bounded
  catch-up collection (1–4 completed periods per call).
- Cancellation accrues completed service and the elapsed part of the current
  period, returning the remaining reserve to customer credit.
- Platform fees use integer arithmetic and cumulative per-subscription rounding;
  splitting collection cannot increase the fee.
- Bilateral early-termination offers bind an exact amount, nonce, digest and
  expiry. Only the merchant can accept the customer's offer. Revocation, another
  collection or extension invalidates the offer.
- Customers, merchants and the platform can each withdraw only their own credit
  to their configured recipient. Failed withdrawals retain credit and require an
  exact principal refund before a new attempt.
- One unresolved outbound globally. Successful settlement debits credit once;
  sorted causal-hash history rejects replayed callbacks.
- Emergency pause blocks new sales and merchant/platform withdrawals while
  preserving customer cancellation and refund withdrawals. Two-step operator
  handover changes authority only after the proposed operator accepts.

Compared with the Procurement Council example, this provides actual customer
refund withdrawals and reuses closed subscription slots, instead of consuming a
new lifetime order slot for every new customer transaction.

## Verification included

**107 Rust tests**: 98 project cases (86 Hub, 12 recipient) plus 9 inherited SDK
unit tests. A deterministic fixture contains **4,096 independent Python-bigint
vectors** for full-width proration and fees. Native test frames use synthetic
signature bytes; actual authentication and callback creation are exercised by
the three real local-stand journeys below.

| Real AVM journey | Assertions |
|---|---|
| Two-asset lifecycle | Repricing isolation, accrual, partial cancellation, negotiated closure, rejected/refunded withdrawal, retry, all three withdrawal roles, pause, operator handover, complete exit and replay |
| Reusable slots | 64 subscriptions opened/closed in four groups of 16; Python oracle checks accrued gross/fees; stale IDs rejected; complete exit and replay |
| Capacity | 4 merchants, 8 plans, 16 active subscriptions, 127 failed/refunded withdrawals followed by a successful 128th; complete exit, funding refusal at the cap and replay |

The first journey deposits `[10,000, 20,000]` simulated units and withdraws exactly
`[10,000, 20,000]`, leaving **zero liabilities**. The churn and capacity journeys
also end with zero liabilities. Capacity uses the stand's three built-in signing
identities for customer wallets; the eight-wallet maximum is separately tested
natively, together with maximum serialized occupancy.

Receipts are in `evidence/avm-*.json`; `evidence/qualification.json` binds sources,
tools, profile, test logs, vectors, artifact digests and journey replay roots.
There is no claim that hosted GitHub CI or an upstream maintainer has approved
this project. A ready-to-run Actions workflow is included.

## Run

Requires Linux x86_64-v3 compatible with the official SDK, Bash, Python 3.11+ and
Rust **1.93.0**, with `wasm32-unknown-unknown`, `rustfmt` and `clippy`.

```bash
python3 scripts/setup_sdk.py
bash scripts/check.sh
```

The setup helper verifies and installs the pinned official SDK under `.tooling`.
The check script obtains **Binaryen 133** if absent, checks its pinned archive
SHA-256, compiles each contract twice with the official builder, optimizes each
compiler artifact twice and compares the two results, then runs all three stand
journeys. Original compiler WASM is retained alongside optimized WASM.

To use an already verified SDK and optimizer:

```bash
bash scripts/check.sh /absolute/path/to/avm-sdk-linux-x86_64 /absolute/path/to/wasm-opt
```

The complete Rust dependency graph is locked and vendored. Once Rust, the SDK
and Binaryen are present, qualification requires no network or signing secrets.
Only the two wasm entrypoint crates install the allocator and panic handler; the
financial core is an rlib dependency, so entrypoints and handlers cannot collide.
All project Rust forbids unsafe code. The separately vendored upstream allocator
contains its normal internal unsafe implementation; it is not a newly invented
allocator or an assertion of whole-dependency safety.

To run only native Rust tests:

```bash
cargo +1.93.0 test --workspace --locked --offline
```

## Amounts and cancellation

For a three-period budget of 3,000, with price 1,000 per 100 time units and a
250-basis-point platform fee, cancellation 50 units after start gives:

- gross service charge: `floor(1000 × 50 / 100) = 500`;
- cumulative fee: `floor(500 × 250 / 10000) = 12`;
- merchant credit: `500 − 12 = 488`;
- released customer reserve: `3000 − 500 = 2500`.

No floating point is used. A withdrawn refund is distinct from an internal
released credit; callbacks confirm only actual ECC withdrawals.

## Files and interfaces

- `core/src/lib.rs`: accounting, timing, consent, offers, callback settlement and codec.
- `contracts/hub`: thin WASM entrypoint/allocator facade and official metadata.
- `contracts/receiver`: bound two-asset recipient fixture with deliberate rejection.
- `scripts/encode.py`: public ABI argument generation and receipt inspection.
- `docs/SPEC.md`: exact binary layouts, rules and error codes.
- `docs/REVIEW.md`: reviewer checklist and SDK inclusion recipe.
- `artifacts/*`: original compiler WASM, optimized WASM, ACAB and descriptor.

```bash
python3 scripts/encode.py plan 0 1000 100
python3 scripts/encode.py join 0 1 3
python3 scripts/encode.py withdraw 0 2500
```

Output is a JSON hex string suitable for `--args-json`.

## Deliberate limits

There are at most **128 withdrawal attempts per deployment**, including failed
attempts. This bounded causal history is a terminal guard, not an automatic
rollover mechanism. Existing liabilities can become unavailable if all attempts
are consumed before they are withdrawn; new deposits, subscriptions and
extensions are refused at the cap. Do not fund beyond the finite local test.
There is no migration or safe timeout-based recovery of an unresolved callback.

The operator is trusted for merchant onboarding and pause liveness, not for
withdrawing another party's credit. Recipient addresses and accepted subscription
prices are fixed. Extensions may retain the old price up to 24 total periods,
even if the plan has since been disabled. Fees apply to service/negotiated award,
not to deposits or unused refunds. The clock comes from AVM execution context;
there is no service-delivery oracle, card integration or recurring external debit.
No merchant revenue guarantee or real-network asset mapping is asserted.

MIT; see `THIRD_PARTY_NOTICES.md` and retained upstream licenses.

## Repository packaging

To keep the Git tree compact, the original Cargo-vendored registry is stored
losslessly as `vendor/registry.tar.xz`. `scripts/check.sh` restores it
automatically when `vendor/registry/` is absent. The three large historical AVM
journey receipts are retained losslessly as `evidence/avm-*.json.xz`; running the
qualification recreates the corresponding JSON receipts. To inspect the archived
receipts directly, use `xz -dk evidence/avm-*.json.xz`.
