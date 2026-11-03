# Review and integration guide

Review the financial specification before the code: price snapshots, extension
rights, cumulative rounding, cancellation and negotiated settlement determine
what users authorize. An operator controls merchant onboarding and pause, while
customers and merchants control their own credit/consent.

Read `core/src/lib.rs` in this order: `State::validate`/`liabilities`, subscription
`due`/`reserve`, `charge`, transition dispatcher, withdrawal callback, binary
codec. WASM facade code is deliberately thin. Project crates forbid unsafe code;
upstream dlmalloc implements the allocator internally. Crate source versions,
licenses and Cargo checksum files are retained.

Run `bash scripts/check.sh SDK_DIR WASM_OPT`. The original compiler artifacts and
two matched optimization outputs are separate proof steps. The final optimized
WASM is what the stand deploys and admits. Both original and optimized bytes are
retained; a compiler artifact fitting the code cap does not alone prove a deploy
fits its message-body cap.

Native tests provide canonical ABI frames with synthetic signatures. They do
not verify signatures. The lifecycle/churn/capacity scripts exercise the actual
stand's authenticated calls, callback/refund construction and journal replay.
The stand has three built-in signing identities, so the maximum eight wallets is
native-tested rather than claimed as an eight-wallet authenticated stand run.

The independent Python bigint fixture tests full-width proration and fee math;
64-subscription churn additionally computes a Python oracle from actual call
timestamps. Sorted receipt validation is intentionally linear, and the largest
callback history is exercised under the unmodified local runtime's FUEL policy.

For SDK inclusion, retain licenses; replace the vendored guest dependency with
the checkout's crate, and requalify after any code, tool, ABI, allocator or profile
change. Keep allocator/panic handlers in the two root WASM facade crates, not the
shared rlib; keep metadata IDs generated from official ACAB. A hosted CI workflow
is supplied but its presence does not establish a hosted pass or maintainer
approval.

Deliberate limits: 8 wallets, 4 merchants, 8 plans, 16 simultaneous subscriptions,
24 periods per subscription, 128 withdrawal attempts, one unresolved outbound.
No key/recipient rotation, migration, timeout recovery, production rollover,
service-delivery oracle, card/autopay integration or mainnet mapping is supplied.

The capacity guard can strand remaining liabilities after excessive failed
withdrawals. Do not treat this finite local example as a production fund holder.
The final stress journey exits all principal on its 128th attempt; that result
is not a proof that arbitrary failure histories can recover.
