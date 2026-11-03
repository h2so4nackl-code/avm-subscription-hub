use avm_contract_sdk::{
    decode_message_input, encode_message_output, Contract, MessageOutput, StateUpdate,
};
use avm_subscription_hub::*;
mod support;
use support::*;
const A: [u8; 32] = [0x41; 32];
const U: [u8; 32] = [0x42; 32];
const M: [u8; 32] = [0x43; 32];
const X: [u8; 32] = [0x44; 32];
fn config() -> Config {
    Config {
        admin: A,
        treasury: address(4),
        assets: [0, 7],
        fee_bps: 250,
        fuel: 10000,
    }
}
#[allow(clippy::too_many_arguments)]
fn eval(
    s: &[u8],
    actor: [u8; 32],
    method: u64,
    body: &[u8],
    asset: u8,
    value: u128,
    time: u32,
    balance: u128,
) -> MessageOutput {
    let extra = if asset == 1 && value != 0 {
        vec![(7, value)]
    } else {
        vec![]
    };
    let msg = external(
        body,
        actor,
        method,
        if asset == 0 { value } else { 0 },
        address(1),
        false,
        &extra,
    );
    let b = frame(&msg, s, actor, None, address(1), time, balance);
    let out = Hub::process_message(decode_message_input(&b).unwrap());
    encode_message_output(&out).unwrap();
    out
}
fn call(s: &[u8], actor: [u8; 32], method: u64, body: &[u8], time: u32) -> MessageOutput {
    eval(s, actor, method, body, 0, 0, time, 1000000)
}
fn state(o: MessageOutput) -> Vec<u8> {
    assert_eq!(o.status_code, 0);
    let Some(StateUpdate::FullReplace(b)) = o.state_update else {
        panic!()
    };
    let s = State::decode(&b).unwrap();
    assert_eq!(s.encode(), b);
    b
}
fn fail(o: MessageOutput, code: u32) {
    assert_eq!(o.status_code, code);
    assert!(
        o.state_update.is_none()
            && o.outbound_intents.is_empty()
            && o.callback_intents.is_empty()
            && o.event_intents.is_empty()
            && o.callback_result_body.is_empty()
    );
}
fn init() -> Vec<u8> {
    let msg = external(
        &envelope(&config().encode()),
        A,
        0,
        0,
        address(1),
        true,
        &[],
    );
    let b = frame(&msg, &[], A, None, address(1), 1, 0);
    state(Hub::init(decode_message_input(&b).unwrap()))
}
fn bytes_address(id: u8) -> Vec<u8> {
    let mut b = vec![];
    put_address(&mut b, address(id));
    b
}
fn plan_args(asset: u8, price: u128, period: u32) -> Vec<u8> {
    let mut b = vec![asset];
    b.extend(price.to_le_bytes());
    b.extend(period.to_le_bytes());
    b
}
fn edit_args(index: u8, version: u32, price: u128, period: u32, active: bool) -> Vec<u8> {
    let mut b = vec![index];
    b.extend(version.to_le_bytes());
    b.extend(price.to_le_bytes());
    b.extend(period.to_le_bytes());
    b.push(active as u8);
    b
}
fn join_args(plan: u8, version: u32, terms: u8) -> Vec<u8> {
    let mut b = vec![plan];
    b.extend(version.to_le_bytes());
    b.push(terms);
    b
}
fn id_more(id: u64, n: u8) -> Vec<u8> {
    let mut b = id.to_le_bytes().to_vec();
    b.push(n);
    b
}
fn withdraw(asset: u8, amount: u128) -> Vec<u8> {
    let mut b = vec![asset];
    b.extend(amount.to_le_bytes());
    b
}
fn offer_args(id: u64, award: u128, until: u32, digest: u8) -> Vec<u8> {
    let mut b = id.to_le_bytes().to_vec();
    b.extend(award.to_le_bytes());
    b.extend(until.to_le_bytes());
    b.extend([digest; 32]);
    b
}
fn accept_args(id: u64, nonce: u32, digest: u8) -> Vec<u8> {
    let mut b = id.to_le_bytes().to_vec();
    b.extend(nonce.to_le_bytes());
    b.extend([digest; 32]);
    b
}
fn opened() -> Vec<u8> {
    let s = state(call(&init(), U, ids::WALLET_OPEN, &bytes_address(2), 2));
    let mut args = M.to_vec();
    args.extend(bytes_address(3));
    state(call(&s, A, ids::MERCHANT_ADD, &args, 3))
}
fn funded() -> Vec<u8> {
    let s = state(eval(&opened(), U, ids::DEPOSIT, &[0], 0, 10000, 4, 10000));
    state(eval(&s, U, ids::DEPOSIT, &[1], 1, 20000, 5, 20000))
}
fn planned() -> Vec<u8> {
    let s = state(call(
        &funded(),
        M,
        ids::PLAN_NEW,
        &plan_args(0, 1000, 100),
        6,
    ));
    state(call(&s, M, ids::PLAN_NEW, &plan_args(1, 2000, 100), 7))
}
fn joined() -> Vec<u8> {
    state(call(&planned(), U, ids::JOIN, &join_args(0, 1, 3), 10))
}
fn cb(
    s: &[u8],
    source: u8,
    body: &[u8],
    status: u32,
    asset: u32,
    value: u128,
    hash: u8,
) -> MessageOutput {
    let mut msg = vec![0; 176];
    msg[1] = 1;
    msg[2..4].copy_from_slice(&u16::from(value != 0).to_le_bytes());
    msg[4..8].copy_from_slice(&(body.len() as u32).to_le_bytes());
    msg[8..16].copy_from_slice(&ids::CALLBACK.to_le_bytes());
    msg[32..40].copy_from_slice(&5u64.to_le_bytes());
    msg[40..44].copy_from_slice(&20u32.to_le_bytes());
    write_address_at(&mut msg, 48, address(source));
    write_address_at(&mut msg, 112, address(1));
    msg.extend([hash; 32]);
    msg.extend(address(1).dapp_id);
    msg.extend(address(1).account_id);
    msg.extend(status.to_le_bytes());
    if value != 0 {
        msg.extend(asset.to_le_bytes());
        msg.extend(value.to_le_bytes());
    }
    msg.extend(body);
    let b = frame(
        &msg,
        s,
        [0; 32],
        Some(address(source)),
        address(1),
        30,
        20000,
    );
    let out = Hub::process_message(decode_message_input(&b).unwrap());
    encode_message_output(&out).unwrap();
    out
}
#[test]
fn config_and_state_roundtrip() {
    assert_eq!(config().encode().len(), CONFIG_BYTES);
    assert_eq!(Config::decode(&config().encode()), Ok(config()));
    State::decode(&init()).unwrap();
}
#[test]
fn deployment_requires_admin() {
    let msg = external(
        &envelope(&config().encode()),
        X,
        0,
        0,
        address(1),
        true,
        &[],
    );
    let b = frame(&msg, &[], X, None, address(1), 1, 0);
    fail(Hub::init(decode_message_input(&b).unwrap()), AUTH);
}
#[test]
fn config_rejects_duplicate_assets() {
    let mut c = config();
    c.assets = [7, 7];
    assert!(Config::decode(&c.encode()).is_err());
}
#[test]
fn config_rejects_shell_principal() {
    let mut c = config();
    c.assets = [0, 2];
    assert!(c.validate().is_err());
}
#[test]
fn config_rejects_fee_above_cap() {
    let mut c = config();
    c.fee_bps = 1001;
    assert!(c.validate().is_err());
}
#[test]
fn config_rejects_zero_authority_and_fuel() {
    for n in 0..2 {
        let mut c = config();
        if n == 0 {
            c.admin = [0; 32];
        } else {
            c.fuel = 0;
        }
        assert!(c.validate().is_err());
    }
}
#[test]
fn wallet_open_rejects_duplicate_and_self_destination() {
    fail(
        call(&opened(), U, ids::WALLET_OPEN, &bytes_address(2), 4),
        PHASE,
    );
    fail(
        call(&init(), U, ids::WALLET_OPEN, &bytes_address(1), 4),
        ARGS,
    );
}
#[test]
fn wallet_open_rejects_cross_dapp() {
    let mut b = bytes_address(2);
    b[0] = 0x77;
    fail(call(&init(), U, ids::WALLET_OPEN, &b, 4), ARGS);
}
#[test]
fn merchant_registration_is_admin_only() {
    let mut b = M.to_vec();
    b.extend(bytes_address(3));
    fail(call(&opened(), U, ids::MERCHANT_ADD, &b, 4), AUTH);
}
#[test]
fn merchant_registration_rejects_duplicate_key() {
    let mut b = M.to_vec();
    b.extend(bytes_address(3));
    fail(call(&opened(), A, ids::MERCHANT_ADD, &b, 4), PHASE);
}
#[test]
fn deposits_keep_asset_ledgers_separate() {
    let s = State::decode(&funded()).unwrap();
    assert_eq!(s.deposited, [10000, 20000]);
    assert_eq!(s.wallets[0].credit, [10000, 20000]);
}
#[test]
fn deposits_require_registered_wallet() {
    fail(eval(&init(), U, ids::DEPOSIT, &[0], 0, 1, 4, 1), AUTH);
}
#[test]
fn deposits_require_exact_positive_asset() {
    fail(call(&opened(), U, ids::DEPOSIT, &[0], 4), VALUE);
    fail(
        eval(&opened(), U, ids::DEPOSIT, &[1], 0, 100, 4, 100),
        VALUE,
    );
    fail(eval(&opened(), U, ids::DEPOSIT, &[2], 0, 100, 4, 100), ARGS);
}
#[test]
fn deposit_rejects_insufficient_actual_balance() {
    fail(
        eval(&opened(), U, ids::DEPOSIT, &[0], 0, 100, 4, 99),
        BALANCE,
    );
}
#[test]
fn deposit_overflow_is_atomic() {
    let mut s = State::decode(&opened()).unwrap();
    s.deposited[0] = u128::MAX;
    s.wallets[0].credit[0] = u128::MAX;
    fail(
        eval(&s.encode(), U, ids::DEPOSIT, &[0], 0, 1, 4, u128::MAX),
        LIMIT,
    );
}
#[test]
fn plan_creation_requires_merchant() {
    fail(
        call(&funded(), U, ids::PLAN_NEW, &plan_args(0, 100, 10), 6),
        AUTH,
    );
}
#[test]
fn plan_rejects_invalid_price_period_and_asset() {
    for args in [
        plan_args(2, 100, 10),
        plan_args(0, 0, 10),
        plan_args(0, 100, 0),
    ] {
        fail(call(&funded(), M, ids::PLAN_NEW, &args, 6), ARGS);
    }
}
#[test]
fn plan_edit_requires_owner_and_expected_version() {
    fail(
        call(
            &planned(),
            M,
            ids::PLAN_EDIT,
            &edit_args(0, 2, 9000, 200, true),
            8,
        ),
        STALE,
    );
    fail(
        call(
            &planned(),
            U,
            ids::PLAN_EDIT,
            &edit_args(0, 1, 9000, 200, true),
            8,
        ),
        AUTH,
    );
}
#[test]
fn edits_do_not_change_existing_subscription_terms() {
    let s = state(call(
        &joined(),
        M,
        ids::PLAN_EDIT,
        &edit_args(0, 1, 9000, 200, true),
        20,
    ));
    let s = State::decode(&s).unwrap();
    assert_eq!((s.plans[0].version, s.plans[0].price), (2, 9000));
    assert_eq!(
        (s.subs[0].version, s.subs[0].price, s.subs[0].period),
        (1, 1000, 100)
    );
}
#[test]
fn join_rejects_stale_price_quote() {
    let s = state(call(
        &planned(),
        M,
        ids::PLAN_EDIT,
        &edit_args(0, 1, 2000, 100, true),
        8,
    ));
    fail(call(&s, U, ids::JOIN, &join_args(0, 1, 1), 10), STALE);
}
#[test]
fn inactive_plan_blocks_new_join() {
    let s = state(call(
        &planned(),
        M,
        ids::PLAN_EDIT,
        &edit_args(0, 1, 1000, 100, false),
        8,
    ));
    fail(call(&s, U, ids::JOIN, &join_args(0, 2, 1), 10), PHASE);
}
#[test]
fn join_reserves_only_selected_asset() {
    let s = State::decode(&joined()).unwrap();
    assert_eq!(s.wallets[0].credit, [7000, 20000]);
    assert_eq!(s.subs[0].reserve(), Ok(3000));
    assert_eq!(s.liabilities(), Ok([10000, 20000]));
}
#[test]
fn join_checks_term_bounds_and_available_credit() {
    for n in [0, 25, 255] {
        fail(
            call(&planned(), U, ids::JOIN, &join_args(0, 1, n), 10),
            ARGS,
        );
    }
    fail(
        call(&planned(), U, ids::JOIN, &join_args(0, 1, 11), 10),
        BALANCE,
    );
}
#[test]
fn join_checks_budget_and_timestamp_overflow() {
    let mut s = State::decode(&planned()).unwrap();
    s.plans[0].price = u128::MAX;
    fail(
        call(&s.encode(), U, ids::JOIN, &join_args(0, 1, 2), 10),
        LIMIT,
    );
    fail(
        call(&planned(), U, ids::JOIN, &join_args(0, 1, 1), u32::MAX - 50),
        TIME,
    );
}
#[test]
fn billing_before_due_is_rejected() {
    fail(call(&joined(), X, ids::COLLECT, &id_more(1, 1), 109), TIME);
}
#[test]
fn billing_due_boundary_splits_fee_exactly() {
    let s = state(call(&joined(), X, ids::COLLECT, &id_more(1, 1), 110));
    let s = State::decode(&s).unwrap();
    assert_eq!(
        (s.subs[0].billed, s.merchants[0].credit[0], s.fees[0]),
        (1000, 975, 25)
    );
}
#[test]
fn collection_is_permissionless_and_cannot_double_bill() {
    let s = state(call(&joined(), X, ids::COLLECT, &id_more(1, 1), 110));
    fail(call(&s, X, ids::COLLECT, &id_more(1, 1), 110), TIME);
}
#[test]
fn bounded_catchup_collects_only_requested_periods() {
    let s = state(call(&planned(), U, ids::JOIN, &join_args(0, 1, 10), 10));
    let s = state(call(&s, X, ids::COLLECT, &id_more(1, 4), 1010));
    let s = State::decode(&s).unwrap();
    assert_eq!(s.subs[0].billed, 4000);
}
#[test]
fn completed_subscription_releases_slot_and_stale_id_fails() {
    let s = state(call(&joined(), X, ids::COLLECT, &id_more(1, 4), 310));
    assert!(State::decode(&s).unwrap().subs.is_empty());
    fail(call(&s, U, ids::CANCEL, &1u64.to_le_bytes(), 311), STALE);
}
#[test]
fn cancellation_at_start_refunds_whole_reserve() {
    let s = state(call(&joined(), U, ids::CANCEL, &1u64.to_le_bytes(), 10));
    let s = State::decode(&s).unwrap();
    assert!(s.subs.is_empty());
    assert_eq!(s.wallets[0].credit, [10000, 20000]);
}
#[test]
fn cancellation_prorates_partial_period() {
    let s = state(call(&joined(), U, ids::CANCEL, &1u64.to_le_bytes(), 60));
    let s = State::decode(&s).unwrap();
    assert_eq!(
        (s.wallets[0].credit[0], s.merchants[0].credit[0], s.fees[0]),
        (9500, 488, 12)
    );
}
#[test]
fn cancellation_catches_all_completed_periods() {
    let s = state(call(&joined(), U, ids::CANCEL, &1u64.to_le_bytes(), 260));
    let s = State::decode(&s).unwrap();
    assert_eq!(
        (s.wallets[0].credit[0], s.merchants[0].credit[0], s.fees[0]),
        (7500, 2438, 62)
    );
}
#[test]
fn cancellation_is_owner_only() {
    fail(
        call(&joined(), M, ids::CANCEL, &1u64.to_le_bytes(), 60),
        AUTH,
    );
}
#[test]
fn cancellation_after_end_never_overcharges() {
    let s = state(call(&joined(), U, ids::CANCEL, &1u64.to_le_bytes(), 500));
    let s = State::decode(&s).unwrap();
    assert_eq!(
        (s.wallets[0].credit[0], s.merchants[0].credit[0], s.fees[0]),
        (7000, 2925, 75)
    );
}
#[test]
fn clock_rewind_rejects() {
    let s = state(call(&joined(), X, ids::COLLECT, &id_more(1, 1), 110));
    fail(call(&s, U, ids::CANCEL, &1u64.to_le_bytes(), 109), TIME);
}
#[test]
fn extension_preserves_original_price_and_invalidates_offer() {
    let s = state(call(
        &joined(),
        U,
        ids::OFFER,
        &offer_args(1, 0, 200, 0x61),
        20,
    ));
    let s = state(call(&s, U, ids::EXTEND, &id_more(1, 2), 21));
    let s = State::decode(&s).unwrap();
    assert_eq!(s.subs[0].terms, 5);
    assert!(s.subs[0].offer.is_none());
    assert_eq!(s.wallets[0].credit[0], 5000);
}
#[test]
fn extension_is_owner_only_and_bounded() {
    fail(call(&joined(), M, ids::EXTEND, &id_more(1, 1), 20), AUTH);
    fail(call(&joined(), U, ids::EXTEND, &id_more(1, 22), 20), LIMIT);
    fail(call(&joined(), U, ids::EXTEND, &id_more(1, 1), 310), TIME);
}
#[test]
fn offer_accrues_until_proposal_and_reserves_exact_award() {
    let s = state(call(
        &joined(),
        U,
        ids::OFFER,
        &offer_args(1, 700, 200, 0x61),
        60,
    ));
    let s = State::decode(&s).unwrap();
    assert_eq!(s.subs[0].billed, 500);
    assert_eq!(s.subs[0].offer.as_ref().unwrap().amount, 700);
}
#[test]
fn offer_checks_remaining_reserve_and_expiry() {
    fail(
        call(
            &joined(),
            U,
            ids::OFFER,
            &offer_args(1, 2501, 200, 0x61),
            60,
        ),
        BALANCE,
    );
    fail(
        call(&joined(), U, ids::OFFER, &offer_args(1, 0, 60, 0x61), 60),
        ARGS,
    );
}
#[test]
fn accepting_offer_closes_with_agreed_split() {
    let s = state(call(
        &joined(),
        U,
        ids::OFFER,
        &offer_args(1, 700, 200, 0x61),
        60,
    ));
    let s = state(call(&s, M, ids::OFFER_ACCEPT, &accept_args(1, 1, 0x61), 70));
    let s = State::decode(&s).unwrap();
    assert!(s.subs.is_empty());
    assert_eq!(
        (s.wallets[0].credit[0], s.merchants[0].credit[0], s.fees[0]),
        (8800, 1170, 30)
    );
}
#[test]
fn offer_requires_counterparty_and_exact_nonce_digest() {
    let s = state(call(
        &joined(),
        U,
        ids::OFFER,
        &offer_args(1, 700, 200, 0x61),
        60,
    ));
    fail(
        call(&s, U, ids::OFFER_ACCEPT, &accept_args(1, 1, 0x61), 70),
        AUTH,
    );
    fail(
        call(&s, M, ids::OFFER_ACCEPT, &accept_args(1, 2, 0x61), 70),
        STALE,
    );
    fail(
        call(&s, M, ids::OFFER_ACCEPT, &accept_args(1, 1, 0x62), 70),
        STALE,
    );
}
#[test]
fn expired_offer_cannot_settle() {
    let s = state(call(
        &joined(),
        U,
        ids::OFFER,
        &offer_args(1, 0, 65, 0x61),
        60,
    ));
    fail(
        call(&s, M, ids::OFFER_ACCEPT, &accept_args(1, 1, 0x61), 66),
        TIME,
    );
}
#[test]
fn revoke_offer_prevents_later_acceptance() {
    let s = state(call(
        &joined(),
        U,
        ids::OFFER,
        &offer_args(1, 0, 200, 0x61),
        60,
    ));
    let s = state(call(&s, U, ids::OFFER_REVOKE, &1u64.to_le_bytes(), 61));
    fail(
        call(&s, M, ids::OFFER_ACCEPT, &accept_args(1, 1, 0x61), 62),
        PHASE,
    );
}
#[test]
fn subsequent_billing_invalidates_offer() {
    let s = state(call(
        &joined(),
        U,
        ids::OFFER,
        &offer_args(1, 0, 200, 0x61),
        60,
    ));
    let s = state(call(&s, X, ids::COLLECT, &id_more(1, 1), 110));
    fail(
        call(&s, M, ids::OFFER_ACCEPT, &accept_args(1, 1, 0x61), 111),
        PHASE,
    );
}
#[test]
fn fee_rounding_is_cumulative_per_subscription() {
    let s = state(call(
        &joined(),
        U,
        ids::OFFER,
        &offer_args(1, 0, 200, 0x61),
        11,
    ));
    let s = state(call(&s, U, ids::CANCEL, &1u64.to_le_bytes(), 60));
    assert_eq!(State::decode(&s).unwrap().fees[0], 12);
}
#[test]
fn pause_blocks_sales_but_preserves_customer_exit() {
    let s = state(call(&joined(), A, ids::PAUSE, &[1], 20));
    fail(call(&s, U, ids::JOIN, &join_args(0, 1, 1), 21), PAUSED);
    fail(call(&s, U, ids::EXTEND, &id_more(1, 1), 21), PAUSED);
    assert_eq!(
        call(&s, U, ids::CANCEL, &1u64.to_le_bytes(), 60).status_code,
        0
    );
}
#[test]
fn pause_is_admin_only() {
    fail(call(&joined(), U, ids::PAUSE, &[1], 20), AUTH);
}
#[test]
fn admin_handover_requires_proposal_and_new_admin_signature() {
    fail(call(&opened(), X, ids::ADMIN_ACCEPT, &[], 10), AUTH);
    let s = state(call(&opened(), A, ids::ADMIN_PROPOSE, &X, 10));
    fail(call(&s, A, ids::ADMIN_ACCEPT, &[], 11), AUTH);
    let s = state(call(&s, X, ids::ADMIN_ACCEPT, &[], 11));
    fail(call(&s, A, ids::PAUSE, &[1], 12), AUTH);
    assert_eq!(call(&s, X, ids::PAUSE, &[1], 12).status_code, 0);
}
#[test]
fn withdrawal_does_not_debit_until_settled() {
    let out = call(&funded(), U, ids::WITHDRAW_CUSTOMER, &withdraw(0, 1000), 20);
    assert_eq!(out.outbound_intents.len(), 1);
    let s = State::decode(&state(out)).unwrap();
    assert_eq!(s.wallets[0].credit[0], 10000);
    assert_eq!(s.withdrawn, [0; 2]);
    assert!(s.pending.is_some());
}
#[test]
fn successful_customer_withdrawal_reduces_only_its_asset() {
    let out = call(&funded(), U, ids::WITHDRAW_CUSTOMER, &withdraw(1, 1234), 20);
    let body = out.outbound_intents[0].body.clone();
    let s = state(cb(&state(out), 2, &body, 0, 0, 0, 0x71));
    let s = State::decode(&s).unwrap();
    assert_eq!(s.withdrawn, [0, 1234]);
    assert_eq!(s.wallets[0].credit, [10000, 18766]);
}
#[test]
fn failed_withdrawal_requires_exact_asset_refund() {
    let s = state(call(
        &funded(),
        U,
        ids::WITHDRAW_CUSTOMER,
        &withdraw(1, 1234),
        20,
    ));
    fail(cb(&s, 2, &[], 4102, 0, 1234, 0x71), CALLBACK);
    fail(cb(&s, 2, &[], 4102, 7, 1233, 0x71), CALLBACK);
    let s = state(cb(&s, 2, &[], 4102, 7, 1234, 0x71));
    assert_eq!(State::decode(&s).unwrap().wallets[0].credit[1], 20000);
}
#[test]
fn wrong_source_or_modified_receipt_cannot_settle() {
    let out = call(&funded(), U, ids::WITHDRAW_CUSTOMER, &withdraw(0, 1000), 20);
    let mut body = out.outbound_intents[0].body.clone();
    let s = state(out);
    fail(cb(&s, 3, &body, 0, 0, 0, 0x71), CALLBACK);
    body[20] ^= 1;
    fail(cb(&s, 2, &body, 0, 0, 0, 0x71), CALLBACK);
}
#[test]
fn replayed_failure_hash_cannot_settle_new_attempt() {
    let s = state(call(
        &funded(),
        U,
        ids::WITHDRAW_CUSTOMER,
        &withdraw(0, 1000),
        20,
    ));
    let s = state(cb(&s, 2, &[], 4102, 0, 1000, 0x71));
    let out = call(&s, U, ids::WITHDRAW_CUSTOMER, &withdraw(0, 1000), 21);
    let body = out.outbound_intents[0].body.clone();
    let s = state(out);
    fail(cb(&s, 2, &body, 0, 0, 0, 0x71), CALLBACK);
    assert_eq!(cb(&s, 2, &body, 0, 0, 0, 0x72).status_code, 0);
}
#[test]
fn pending_blocks_repeat_withdrawals_and_financial_mutations() {
    let s = state(call(
        &joined(),
        U,
        ids::WITHDRAW_CUSTOMER,
        &withdraw(0, 100),
        20,
    ));
    for method in [
        ids::JOIN,
        ids::CANCEL,
        ids::COLLECT,
        ids::WITHDRAW_CUSTOMER,
        ids::ADMIN_PROPOSE,
    ] {
        fail(call(&s, U, method, &[], 21), PENDING);
    }
}
#[test]
fn pause_can_be_set_during_pending_and_callback_still_settles() {
    let out = call(&funded(), U, ids::WITHDRAW_CUSTOMER, &withdraw(0, 100), 20);
    let body = out.outbound_intents[0].body.clone();
    let s = state(call(&state(out), A, ids::PAUSE, &[1], 21));
    let s = state(cb(&s, 2, &body, 0, 0, 0, 0x71));
    assert!(State::decode(&s).unwrap().paused);
}
#[test]
fn merchants_cannot_withdraw_customer_or_protocol_liabilities() {
    let s = state(call(&joined(), X, ids::COLLECT, &id_more(1, 1), 110));
    fail(
        call(&s, M, ids::WITHDRAW_CUSTOMER, &withdraw(0, 1), 111),
        AUTH,
    );
    fail(call(&s, M, ids::WITHDRAW_FEES, &withdraw(0, 1), 111), AUTH);
    fail(
        call(&s, M, ids::WITHDRAW_MERCHANT, &withdraw(0, 976), 111),
        BALANCE,
    );
}
#[test]
fn merchant_and_fee_withdrawal_destinations_are_distinct() {
    let s = state(call(&joined(), X, ids::COLLECT, &id_more(1, 1), 110));
    let out = call(&s, M, ids::WITHDRAW_MERCHANT, &withdraw(0, 975), 111);
    assert_eq!(out.outbound_intents[0].destination, address(3));
    let out = call(&s, A, ids::WITHDRAW_FEES, &withdraw(0, 25), 111);
    assert_eq!(out.outbound_intents[0].destination, address(4));
}
#[test]
fn unsolicited_value_rejects_ordinary_methods() {
    fail(
        eval(
            &joined(),
            U,
            ids::CANCEL,
            &1u64.to_le_bytes(),
            0,
            1,
            60,
            20000,
        ),
        VALUE,
    );
}
#[test]
fn balances_are_checked_before_dispatch() {
    fail(
        eval(
            &funded(),
            U,
            ids::WITHDRAW_CUSTOMER,
            &withdraw(0, 1),
            0,
            0,
            20,
            9999,
        ),
        BALANCE,
    );
}
#[test]
fn attempt_limit_stops_new_deposits_and_withdrawals() {
    let mut s = State::decode(&funded()).unwrap();
    s.attempts = 128;
    s.receipts = (1..=128).map(|x| [x; 32]).collect();
    fail(
        call(&s.encode(), U, ids::WITHDRAW_CUSTOMER, &withdraw(0, 1), 20),
        LIMIT,
    );
    fail(
        eval(&s.encode(), U, ids::DEPOSIT, &[0], 0, 1, 20, 30000),
        LIMIT,
    );
}
#[test]
fn subscription_slots_are_reusable_without_reusing_ids() {
    let mut b = planned();
    for id in 1u64..=40 {
        b = state(call(&b, U, ids::JOIN, &join_args(0, 1, 1), 10));
        b = state(call(&b, U, ids::CANCEL, &id.to_le_bytes(), 10));
    }
    assert_eq!(State::decode(&b).unwrap().next_id, 41);
    fail(call(&b, U, ids::CANCEL, &1u64.to_le_bytes(), 11), STALE);
}
#[test]
fn corrupt_padding_and_lengths_reject() {
    let s = joined();
    for n in [0, 1, STATE_BYTES - 1, STATE_BYTES + 1] {
        assert!(State::decode(&vec![0; n]).is_err());
    }
    let mut b = s;
    b[STATE_BYTES - 1] = 1;
    assert!(State::decode(&b).is_err());
}
#[test]
fn liability_tampering_rejects() {
    let mut s = State::decode(&joined()).unwrap();
    s.wallets[0].credit[0] += 1;
    assert!(s.validate().is_err());
}
#[test]
fn single_byte_mutations_never_panic_or_accept_noncanonical_state() {
    let s = joined();
    for i in 0..STATE_BYTES {
        let mut b = s.clone();
        b[i] ^= 0xff;
        if let Ok(v) = State::decode(&b) {
            assert_eq!(v.encode(), b);
        }
    }
}
#[test]
fn fee_and_proration_match_independent_integer_reference() {
    for price in [1, 3, 39, 40, 999, 1000, 10001, u64::MAX as u128] {
        for elapsed in 0..301u128 {
            let expected = price * elapsed / 100;
            assert_eq!(mul_div(price, elapsed, 100), Ok(expected));
            assert_eq!(mul_div(expected, 250, 10000), Ok(expected * 250 / 10000));
        }
    }
}
#[test]
fn max_u128_proration_avoids_naive_product_overflow() {
    assert_eq!(mul_div(u128::MAX, 50, 100), Ok(u128::MAX / 2));
    assert_eq!(mul_div(u128::MAX, 1000, 10000), Ok(u128::MAX / 10));
}
#[test]
fn malformed_calls_return_no_effects() {
    let b = joined();
    for method in [
        ids::JOIN,
        ids::CANCEL,
        ids::COLLECT,
        ids::OFFER,
        ids::OFFER_ACCEPT,
        ids::WITHDRAW_CUSTOMER,
    ] {
        for n in [0, 1, 2, 100] {
            let o = call(&b, U, method, &vec![0; n], 20);
            assert_ne!(o.status_code, 0);
            assert!(o.state_update.is_none() && o.outbound_intents.is_empty());
        }
    }
}
#[test]
fn wallet_capacity_has_explicit_failure() {
    let mut b = init();
    for k in 1..=8 {
        b = state(call(&b, [k; 32], ids::WALLET_OPEN, &bytes_address(2), 2));
    }
    fail(call(&b, X, ids::WALLET_OPEN, &bytes_address(2), 2), LIMIT);
}
#[test]
fn merchant_capacity_has_explicit_failure() {
    let mut b = init();
    for k in 1..=4 {
        let mut args = vec![k; 32];
        args.extend(bytes_address(3));
        b = state(call(&b, A, ids::MERCHANT_ADD, &args, 2));
    }
    let mut args = X.to_vec();
    args.extend(bytes_address(3));
    fail(call(&b, A, ids::MERCHANT_ADD, &args, 2), LIMIT);
}
#[test]
fn plan_capacity_still_allows_version_edits() {
    let mut b = funded();
    for _ in 0..8 {
        b = state(call(&b, M, ids::PLAN_NEW, &plan_args(0, 1, 100), 6));
    }
    fail(call(&b, M, ids::PLAN_NEW, &plan_args(0, 1, 100), 6), LIMIT);
    assert_eq!(
        call(&b, M, ids::PLAN_EDIT, &edit_args(0, 1, 2, 100, true), 7).status_code,
        0
    );
}
#[test]
fn concurrent_subscription_capacity_and_slot_reuse() {
    let mut b = planned();
    b = state(call(
        &b,
        M,
        ids::PLAN_EDIT,
        &edit_args(0, 1, 1, 100, true),
        8,
    ));
    for _ in 0..16 {
        b = state(call(&b, U, ids::JOIN, &join_args(0, 2, 1), 10));
    }
    fail(call(&b, U, ids::JOIN, &join_args(0, 2, 1), 10), LIMIT);
    b = state(call(&b, U, ids::CANCEL, &1u64.to_le_bytes(), 10));
    b = state(call(&b, U, ids::JOIN, &join_args(0, 2, 1), 10));
    assert_eq!(State::decode(&b).unwrap().subs.last().unwrap().id, 17);
}
#[test]
fn maximum_state_occupancy_is_canonical_and_bounded() {
    let mut s = State::new(config());
    for n in 1..=8 {
        s.wallets.push(Wallet {
            owner: [n; 32],
            destination: address(2),
            credit: [0; 2],
        });
    }
    for n in 20..24 {
        s.merchants.push(Wallet {
            owner: [n; 32],
            destination: address(3),
            credit: [0; 2],
        });
    }
    for _ in 0..8 {
        s.plans.push(Plan {
            merchant: 0,
            asset: 0,
            price: 1,
            period: 1,
            version: 1,
            active: true,
        });
    }
    for id in 1..=16 {
        s.subs.push(Subscription {
            id,
            wallet: 0,
            plan: 0,
            merchant: 0,
            asset: 0,
            price: 1,
            period: 1,
            version: 1,
            start: 1,
            terms: 1,
            billed: 0,
            last_time: 1,
            offer_nonce: 0,
            offer: None,
        });
    }
    s.next_id = 17;
    s.deposited[0] = 16;
    s.attempts = 128;
    s.receipts = (1..=128).map(|x| [x; 32]).collect();
    s.validate().unwrap();
    assert_eq!(s.encode().len(), STATE_BYTES);
    assert_eq!(State::decode(&s.encode()), Ok(s));
}
#[test]
fn fee_reserve_must_be_backed_by_shell() {
    let mut s = State::decode(&funded()).unwrap();
    s.config.fuel = 1000001;
    fail(
        call(&s.encode(), U, ids::WITHDRAW_CUSTOMER, &withdraw(0, 1), 20),
        BALANCE,
    );
}
#[test]
fn withdrawal_rejects_zero_or_invalid_asset() {
    fail(
        call(&funded(), U, ids::WITHDRAW_CUSTOMER, &withdraw(0, 0), 20),
        ARGS,
    );
    fail(
        call(&funded(), U, ids::WITHDRAW_CUSTOMER, &withdraw(2, 1), 20),
        ARGS,
    );
}
#[test]
fn zero_platform_fee_credits_whole_service_amount() {
    let mut s = State::decode(&joined()).unwrap();
    s.config.fee_bps = 0;
    let s = State::decode(&state(call(
        &s.encode(),
        U,
        ids::CANCEL,
        &1u64.to_le_bytes(),
        60,
    )))
    .unwrap();
    assert_eq!(s.merchants[0].credit[0], 500);
    assert_eq!(s.fees[0], 0);
}
#[test]
fn maximum_allowed_platform_fee_is_exact() {
    let mut s = State::decode(&joined()).unwrap();
    s.config.fee_bps = 1000;
    let s = State::decode(&state(call(
        &s.encode(),
        U,
        ids::CANCEL,
        &1u64.to_le_bytes(),
        60,
    )))
    .unwrap();
    assert_eq!(s.merchants[0].credit[0], 450);
    assert_eq!(s.fees[0], 50);
}
#[test]
fn offer_acceptance_at_deadline_is_inclusive() {
    let s = state(call(
        &joined(),
        U,
        ids::OFFER,
        &offer_args(1, 0, 65, 0x61),
        60,
    ));
    assert_eq!(
        call(&s, M, ids::OFFER_ACCEPT, &accept_args(1, 1, 0x61), 65).status_code,
        0
    );
}
#[test]
fn replacing_offer_changes_nonce_even_with_same_digest() {
    let s = state(call(
        &joined(),
        U,
        ids::OFFER,
        &offer_args(1, 0, 200, 0x61),
        60,
    ));
    let s = state(call(&s, U, ids::OFFER, &offer_args(1, 1, 200, 0x61), 61));
    fail(
        call(&s, M, ids::OFFER_ACCEPT, &accept_args(1, 1, 0x61), 62),
        STALE,
    );
    assert_eq!(
        call(&s, M, ids::OFFER_ACCEPT, &accept_args(1, 2, 0x61), 62).status_code,
        0
    );
}
#[test]
fn cooperative_zero_award_waives_unearned_future_budget() {
    let s = state(call(
        &joined(),
        U,
        ids::OFFER,
        &offer_args(1, 0, 200, 0x61),
        60,
    ));
    let s = state(call(
        &s,
        M,
        ids::OFFER_ACCEPT,
        &accept_args(1, 1, 0x61),
        190,
    ));
    let s = State::decode(&s).unwrap();
    assert_eq!(s.wallets[0].credit[0], 9500);
    assert_eq!(s.merchants[0].credit[0] + s.fees[0], 500);
}
#[test]
fn successful_merchant_and_fee_callbacks_reduce_correct_credit() {
    let s = state(call(&joined(), X, ids::COLLECT, &id_more(1, 1), 110));
    for (method, actor, amount, source) in [
        (ids::WITHDRAW_MERCHANT, M, 975, 3),
        (ids::WITHDRAW_FEES, A, 25, 4),
    ] {
        let out = call(&s, actor, method, &withdraw(0, amount), 111);
        let body = out.outbound_intents[0].body.clone();
        let after = state(cb(&state(out), source, &body, 0, 0, 0, 0x71));
        let after = State::decode(&after).unwrap();
        assert_eq!(after.withdrawn[0], amount);
        if source == 3 {
            assert_eq!(after.merchants[0].credit[0], 0);
        } else {
            assert_eq!(after.fees[0], 0);
        }
    }
}
#[test]
fn callback_without_pending_fails() {
    fail(cb(&funded(), 2, &[], 4102, 0, 1, 0x71), CALLBACK);
}
#[test]
fn success_callback_must_not_return_principal() {
    let out = call(&funded(), U, ids::WITHDRAW_CUSTOMER, &withdraw(0, 100), 20);
    let body = out.outbound_intents[0].body.clone();
    fail(cb(&state(out), 2, &body, 0, 0, 100, 0x71), CALLBACK);
}
#[test]
fn admin_pause_does_not_block_customer_withdrawal() {
    let s = state(call(&funded(), A, ids::PAUSE, &[1], 20));
    assert_eq!(
        call(&s, U, ids::WITHDRAW_CUSTOMER, &withdraw(0, 1), 21).status_code,
        0
    );
    fail(call(&s, A, ids::WITHDRAW_FEES, &withdraw(0, 1), 21), PAUSED);
}
#[test]
fn independent_python_bigint_vectors_match() {
    for line in include_str!("fixtures/proration.csv").lines().skip(1) {
        let v: Vec<u128> = line.split(',').map(|n| n.parse().unwrap()).collect();
        assert_eq!(mul_div(v[0], v[1], v[2]), Ok(v[3]));
        assert_eq!(mul_div(v[3], v[4], 10000), Ok(v[5]));
    }
}
#[test]
fn sorted_receipt_history_rejects_noncanonical_order() {
    let mut s = State::decode(&funded()).unwrap();
    s.attempts = 2;
    s.receipts = vec![[2; 32], [1; 32]];
    assert!(s.validate().is_err());
    s.receipts.swap(0, 1);
    assert!(s.validate().is_ok());
}
#[test]
fn new_causal_hash_is_inserted_in_canonical_position() {
    let s = state(call(
        &funded(),
        U,
        ids::WITHDRAW_CUSTOMER,
        &withdraw(0, 100),
        20,
    ));
    let s = state(cb(&s, 2, &[], 4102, 0, 100, 0x72));
    let out = call(&s, U, ids::WITHDRAW_CUSTOMER, &withdraw(0, 100), 21);
    let body = out.outbound_intents[0].body.clone();
    let s = state(cb(&state(out), 2, &body, 0, 0, 0, 0x60));
    assert_eq!(
        State::decode(&s).unwrap().receipts,
        vec![[0x60; 32], [0x72; 32]]
    );
}
