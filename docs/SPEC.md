# Subscription Hub v1

## Trust, assets and authority

The execution clock and authenticated external caller come from AVM context.
There is no off-chain service-delivery oracle. A subscription accepts a pricing
schedule; this contract accounts for time and consent, not service quality.

Configuration contains an operator key, fixed platform recipient, two sorted,
distinct ECC currency IDs (neither may be SHELL ID 2), a fee in 0–1,000 basis
points, and a positive outbound SHELL reserve. The operator registers merchant
keys/recipients, pauses sales/merchant withdrawals, and proposes a successor.
Only the proposed successor can accept; the operator cannot debit customer or
merchant credit. Recipient bindings are fixed in v1; changing operator does not
redirect the platform recipient. The operator is trusted for pause/onboarding
liveness. Customer cancellation and free-credit withdrawals work during pause.

SHELL is used for execution/rent and is never accounted as subscription principal.
All stand journeys use simulated asset IDs 0 and 7. Configurable IDs are not an
assertion about network token mappings or network fee/rent parameters.

## State transitions

1. A customer opens a wallet bound to its own authenticated signing key and a
   nonzero same-DApp recipient different from the Hub. The operator separately
   registers a merchant key and recipient. Duplicate keys within each role fail.
2. A merchant creates a plan (asset index, price per period, positive period).
   Updates require the current version and increment it with checked arithmetic;
   price, period and enabled status can change. Merchant and currency cannot.
3. A customer deposits exactly one positive configured ECC asset into free credit.
   The actual observed asset balance must back recorded net deposits.
4. Joining signs plan ID, expected version and 1–24 periods. The complete budget
   is reserved immediately. Price, period, version and asset are copied into the
   subscription and remain fixed. IDs are monotonic u64 values; closed records
   are removed, freeing concurrent slots without reusing IDs.
5. Anyone can collect 1–4 completed periods. Collection is bounded catch-up,
   not a demand to debit external funds. It allocates earned gross from reserved
   balance to merchant net credit and platform fee credit. The same interval
   cannot be charged twice.
6. The customer can extend before the original/current end by more prepaid
   periods, at the stored price, up to 24 total periods. End timestamps and budgets
   are checked. An extension invalidates an outstanding termination offer.
7. Customer cancellation computes all earned service through the current time,
   including the elapsed fraction of the current period. It closes the record
   and releases unused reserve to that customer's free credit. It is not itself
   an external withdrawal. Cancellation after the end caps service at full budget.
8. Customer, merchant or operator can withdraw positive available credit for its
   role to that role's fixed recipient. The whole recorded principal liability
   in both currencies must be backed, and SHELL reserve must be available.

No method can overdraw another role, change a reserved price, or count a pending
outbound as a completed withdrawal. Unexpected principal on ordinary methods
fails; `topup` accepts only positive SHELL entries. Unaccounted unsolicited assets
are not made withdrawable by a public sweep function.

## Financial formulas

For elapsed time `t = clamp(now − start, 0, period × terms)`:

`gross_due = floor(price × t / period)`.

Permissionless collection advances only completed periods and at most four per
call; cancellation and an offer proposal synchronize earned service to exact
current time. A backward time relative to subscription `last_time` fails.

The helper splits quotient and remainder to avoid a naive full-width `price × t`
intermediate. Its admitted callers bound time numerator/denominator to u32 and
basis-point numerator to 0–1,000. It is not advertised as an arbitrary-precision
mul/div routine for unconstrained u128 numerators. Independent Python bigint
vectors cover 4,096 full-width price cases within these admitted bounds.

Cumulative fee for an individual subscription is:

`fee(gross) = floor(gross × fee_bps / 10000)`.

For any charge increment, `fee_delta = fee(new_gross) − fee(old_gross)` and
`merchant_delta = gross_delta − fee_delta`. Thus collection splitting cannot
increase that subscription's fee. Rounding is per subscription, not across all
customers; no floating-point values or estimated network conversion is involved.

For each asset independently:

`deposited − withdrawn = wallet_free_credit + merchant_credit + platform_fees
                        + active_subscription_reserves`.

The state validates this equality with checked u128 arithmetic. A pending
withdrawal remains part of its source credit and the liability until confirmed.
The recorded total never includes SHELL fees or rent.

## Cooperative early termination

The customer first synchronizes time-based earnings to the offer timestamp,
then proposes an additional merchant gross award from remaining reserve, a
nonzero digest and an expiry within one day. A checked nonce increments on each
new offer. The digest can reference an off-chain agreement; its meaning is not
verified by the contract.

Only that subscription's merchant may accept the exact nonce/digest before or at
the expiry. Acceptance pays the fixed additional award, applies the cumulative
fee difference, releases the rest to customer credit, and closes the subscription.
It intentionally does not accrue more service between proposal and acceptance:
the merchant explicitly accepts the proposed fixed split. The customer may
revoke an offer or use normal cancellation instead. Further billing or extension
invalidates it. Plan repricing does not modify the accepted subscription/offer.

## Asynchronous withdrawal settlement

Only one withdrawal can be pending across the Hub. Dispatch writes pending state
and emits one ECC transfer with a callback request. All financial mutations and
operator handover are locked while pending. Status, positive SHELL topup and
operator pause remain available. A pause cannot recall an emitted transfer.

A callback must have the runtime's callback header, the expected handler method,
exact recipient source, this Hub as cause address, a nonzero unseen causal hash,
and no extra callback request. Success must echo the exact 80-byte receipt and
return no principal. Only then is source credit debited and `withdrawn` incremented.
Failure must have an empty body and return exactly the pending asset and amount;
credit remains unchanged. Both outcomes retain the causal hash and clear pending.
Different/wrong asset, under/over-refund, wrong source, modified receipt and replay
are rejected without effects.

Causal hashes are sorted lexicographically. Decoding checks strictly increasing,
nonzero entries in linear time; lookup/insertion uses binary search followed by
bounded vector insertion. The maximum-history stand case tests all 128 receipts
with 16 concurrent subscriptions. This replaces a quadratic duplicate check that
proved too costly in the development capacity probe.

There is no timeout-based unlock, unauthenticated recovery or guessing about a
missing callback. If settlement is unresolved, the lock remains. A finite history
allows at most 128 attempts, including failures, per deployment. At the cap,
new deposits, joins and extensions fail; prior liabilities can still be stranded
if the budget was not withdrawn before attempts ran out. This is an explicit
reference-example boundary, not a production rollover or recovery design.

## Binary layouts

All integers are little-endian. Addresses are raw DApp[32] || account[32]. Bodies
must have exactly their ABI length; booleans must be 0 or 1. Constructor payload
is the inner init data of the SDK's deploy envelope, not the whole envelope.

Config[122]: admin[32], treasury[64], currency IDs[2 × u32], fee_bps:u16, fuel:u128.

Treasury state is fixed at 12,288 data bytes. Header:

| Offset | Field |
|---:|---|
| 0 | `ASH1` magic[4] |
| 4 | config[122] |
| 126 | deposited[2 × u128] |
| 158 | withdrawn[2 × u128] |
| 190 | platform fee credits[2 × u128] |
| 222 | next subscription ID:u64 |
| 230 | attempt sequence:u64 |
| 238 | paused:u8 |
| 239 | proposed operator[32] |
| 271 | wallet, merchant, plan, subscription, receipt counts:5 × u8 |
| 276 | pending record[27] |
| 303 | variable count-bounded records |

Pending[27]: kind:u8, owner_index:u8, asset_index:u8, attempt:u64, amount:u128.
An idle pending record must be all zero. Kinds: 1 customer, 2 merchant, 3 platform;
platform owner index must be zero.

Wallet and merchant records are each 128 bytes: key[32], recipient[64],
credits[2 × u128]. Plans are 27 bytes: merchant:u8, asset:u8, version:u32,
price:u128, period:u32, enabled:u8.

Subscription[121]: id:u64, wallet:u8, plan:u8, merchant:u8, asset:u8, price:u128,
period:u32, version:u32, start:u32, terms:u32, billed:u128, last_time:u32,
offer_nonce:u32, has_offer:u8, offer_award:u128, offer_until:u32, offer_digest[32].
Absent offer payload must be zero. Receipt hashes[32] follow subscriptions; all
remaining state bytes are zero. Maximum used size is
`303 + (8+4)×128 + 8×27 + 16×121 + 128×32 = 8,087` bytes.

Receipt[80]: `SUBP`[4], attempt:u64, kind:u8, asset_index:u8, currency_id:u32,
owner_index:u8, zero:u8, amount:u128, authenticated source-role key[32], zero[12].
The owner key binds the intent but is not used as a substitute for runtime
callback authentication. Recipient fixture state[192] stores its deployer key,
bound Hub address, last sequence, two received totals, payment count, currency
IDs, a test rejection bit and zero padding. Binding is owner-only and one-time,
which avoids circular constructor addresses.

## Method bodies

All names start with `subscriptions.`.

| Suffix | Body |
|---|---|
| `deploy` | config[122] |
| `wallet.open` | recipient[64] |
| `merchant.add` | merchant key[32], recipient[64] |
| `plan.new` | asset:u8, price:u128, period:u32 |
| `plan.edit` | index:u8, expected_version:u32, price:u128, period:u32, enabled:u8 |
| `deposit` | asset_index:u8; actual value in extra currencies |
| `join` | plan:u8, expected_version:u32, periods:u8 |
| `extend`, `collect` | subscription_id:u64, periods:u8 |
| `cancel`, `offer.revoke` | subscription_id:u64 |
| `offer` | id:u64, award:u128, until:u32, digest[32] |
| `offer.accept` | id:u64, nonce:u32, digest[32] |
| `withdraw.customer`, `withdraw.merchant`, `withdraw.fees` | asset_index:u8, amount:u128 |
| `pause` | bool:u8 |
| `admin.propose` | successor key[32] |
| `admin.accept`, `status`, `topup` | empty |
| `callback` | success receipt[80] or runtime failure callback with empty body |

The companion has `subscriptions.receiver.deploy` with two u32 currency IDs,
`.bind` with Hub address[64], `.reject` with bool:u8 and `.receive` with receipt[80].

## Events and errors

Canonical external event `subscriptions.changed` carries sorted fields
`asset0_liability:u128`, `asset1_liability:u128`, `kind:u64`, `subscription:u64`.
Kinds 0–19: deployment, status, topup, pause, operator proposal, acceptance,
wallet/merchant registration, plan creation, plan edit, deposit, join, withdrawal
pending, collect, offer acceptance, extend, cancel, offer revocation, offer
proposal, successful withdrawal, refunded withdrawal failure.

Hub statuses 4000–4011: invalid args, unauthorized, invalid state, invalid phase,
unexpected value, invalid time, pending transfer, invalid callback, capacity/
overflow, insufficient balance, paused, stale reference. Recipient statuses
4100–4102: unauthorized, invalid args, deliberate rejection. The published ABI
metadata lists these diagnostics. Failures emit no guest effects. Protocol fees
or failure refunds on a real network are separate runtime semantics, not a
promise inferred from local CLI rollback.
