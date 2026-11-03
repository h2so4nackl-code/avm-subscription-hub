#![forbid(unsafe_code)]
#![cfg_attr(target_arch = "wasm32", no_std)]
pub use avm_subscription_core::*;
#[cfg(all(target_arch = "wasm32", feature = "guest"))]
avm_contract_sdk::export_message_contract!(Hub, arena_bytes = 131072);
#[cfg(all(target_arch = "wasm32", feature = "guest"))]
#[global_allocator]
static ALLOCATOR: dlmalloc::GlobalDlmalloc = dlmalloc::GlobalDlmalloc;
#[cfg(all(target_arch = "wasm32", feature = "guest"))]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    core::arch::wasm32::unreachable()
}
