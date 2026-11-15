// Nexus 0.3.6 defect, no libaw: deserialize a 25-key JSON object into an Fx-hashed hashbrown
// map held in a static once-cell, then look one key up. With lto = true and codegen-units = 1
// the guest prints "parsed 28 units" (wrong; the object has 25 keys) and then spins forever in
// hashbrown's probe. The same parse without the static, or the same keys inserted by hand, is
// correct. Adding a few prints turns the hang into a silent early exit, so the trigger depends
// on code layout (an emulator instruction bug or a miscompile; not the heap layout, which
// starts after _end).
#![cfg_attr(target_arch = "riscv32", no_std, no_main)]
extern crate alloc;
use alloc::{string::String, vec::Vec};
mod fx;

#[derive(serde::Deserialize)]
struct UnitInfo {
    cost: u32,
}

struct Once<T>(core::cell::OnceCell<T>);
unsafe impl<T> Sync for Once<T> {}

fn units() -> &'static fx::FxMap<String, UnitInfo> {
    static T: Once<fx::FxMap<String, UnitInfo>> = Once(core::cell::OnceCell::new());
    T.0.get_or_init(|| serde_json::from_str(include_str!("units.json")).unwrap())
}

#[nexus_rt::main]
#[nexus_rt::private_input(input)]
fn main(input: Vec<u8>) -> u32 {
    nexus_rt::println!("input {} bytes", input.len());
    let u = units();
    nexus_rt::println!("parsed {} units", u.len());
    let c = u.get("Infantry").map_or(0, |i| i.cost);
    nexus_rt::println!("Infantry costs {c}");
    c
}
