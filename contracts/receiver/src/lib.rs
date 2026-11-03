#![forbid(unsafe_code)]
#![cfg_attr(target_arch = "wasm32", no_std)]
extern crate alloc;
use alloc::vec;
use avm_contract_sdk::{
    Caller, CanonicalMessage, Contract, MessageInput, MessageOutput, StateUpdate,
};
use avm_subscription_hub::{currency, ids, valid_address, Reader, SHELL};
pub const STATE_BYTES: usize = 192;
pub const AUTH: u32 = 4100;
pub const ARGS: u32 = 4101;
pub const REJECT: u32 = 4102;
pub struct Receiver;
impl Contract for Receiver {
    fn init(i: MessageInput<'_>) -> MessageOutput {
        initialize(i).unwrap_or_else(MessageOutput::failure)
    }
    fn process_message(i: MessageInput<'_>) -> MessageOutput {
        process(i).unwrap_or_else(MessageOutput::failure)
    }
}
fn initialize(i: MessageInput<'_>) -> Result<MessageOutput, u32> {
    let Caller::External(owner) = i.caller else {
        return Err(AUTH);
    };
    let d = i.deploy().map_err(|_| ARGS)?;
    if !i.state.is_empty() || !i.message.extra().is_empty() || d.init_data().len() != 8 {
        return Err(ARGS);
    }
    let mut r = Reader::new(d.init_data());
    let assets = [r.u32()?, r.u32()?];
    if assets[0] >= assets[1] || assets.contains(&SHELL) {
        return Err(ARGS);
    }
    let mut b = vec![0; STATE_BYTES];
    b[..32].copy_from_slice(&owner);
    b[144..152].copy_from_slice(d.init_data());
    Ok(MessageOutput::success(StateUpdate::FullReplace(b)))
}
fn process(i: MessageInput<'_>) -> Result<MessageOutput, u32> {
    let b = i.state;
    if b.len() != STATE_BYTES
        || b[..32] == [0; 32]
        || b[152] > 1
        || b[153..].iter().any(|v| *v != 0)
    {
        return Err(ARGS);
    }
    let mut r = Reader::new(&b[144..152]);
    let assets = [r.u32()?, r.u32()?];
    if assets[0] >= assets[1] || assets.contains(&SHELL) {
        return Err(ARGS);
    }
    let mut state = b.to_vec();
    let m = i.message.method_id();
    if m == ids::BIND || m == ids::REJECT {
        let owner: [u8; 32] = b[..32].try_into().map_err(|_| ARGS)?;
        if i.caller != Caller::External(owner) {
            return Err(AUTH);
        }
        if !i.message.extra().is_empty() {
            return Err(ARGS);
        }
        let body = i.message.body();
        if m == ids::BIND {
            if body.len() != 64 || b[32..96] != [0; 64] {
                return Err(ARGS);
            }
            let bound = Reader::new(body).address()?;
            if !valid_address(bound)
                || bound == i.destination
                || bound.dapp_id != i.destination.dapp_id
            {
                return Err(ARGS);
            }
            state[32..96].copy_from_slice(body);
        } else {
            if body.len() != 1 || body[0] > 1 {
                return Err(ARGS);
            }
            state[152] = body[0];
        }
        return Ok(MessageOutput::success(StateUpdate::FullReplace(state)));
    }
    let (Caller::Internal(source), CanonicalMessage::Internal(msg)) = (i.caller, i.message) else {
        return Err(AUTH);
    };
    let bound = Reader::new(&b[32..96]).address()?;
    if !valid_address(bound)
        || source != bound
        || m != ids::RECEIVE
        || msg.is_callback()
        || !msg.has_callback_request()
    {
        return Err(AUTH);
    }
    if b[152] != 0 {
        return Err(REJECT);
    }
    let body = msg.body;
    if body.len() != 80
        || &body[..4] != b"SUBP"
        || !(1..=3).contains(&body[12])
        || body[13] > 1
        || body[19] != 0
        || body[36..68] == [0; 32]
        || body[68..] != [0; 12]
    {
        return Err(ARGS);
    }
    let mut r = Reader::new(&body[4..]);
    let attempt = r.u64()?;
    let kind = r.u8()?;
    let a = r.u8()? as usize;
    let asset = r.u32()?;
    let owner = r.u8()?;
    if (kind == 1 && owner >= 8)
        || (kind == 2 && owner >= 4)
        || (kind == 3 && owner != 0)
        || asset != assets[a]
    {
        return Err(ARGS);
    }
    r.u8()?;
    let amount = r.u128()?;
    let mut old = Reader::new(&b[96..144]);
    let last = old.u64()?;
    let received = [old.u128()?, old.u128()?];
    let count = old.u64()?;
    if attempt <= last
        || amount == 0
        || msg.extra.len() != 1
        || currency(msg.extra, asset) != amount
    {
        return Err(ARGS);
    }
    state[96..104].copy_from_slice(&attempt.to_le_bytes());
    let total = received[a].checked_add(amount).ok_or(ARGS)?;
    state[104 + a * 16..120 + a * 16].copy_from_slice(&total.to_le_bytes());
    state[136..144].copy_from_slice(&count.checked_add(1).ok_or(ARGS)?.to_le_bytes());
    let mut out = MessageOutput::success(StateUpdate::FullReplace(state));
    out.callback_result_body = body.to_vec();
    Ok(out)
}
#[cfg(target_arch = "wasm32")]
avm_contract_sdk::export_message_contract!(Receiver, arena_bytes = 131072);

#[cfg(target_arch = "wasm32")]
#[global_allocator]
static ALLOCATOR: dlmalloc::GlobalDlmalloc = dlmalloc::GlobalDlmalloc;
#[cfg(target_arch = "wasm32")]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    core::arch::wasm32::unreachable()
}
