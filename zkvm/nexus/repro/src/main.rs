// Nexus 0.3.6: read a private input, then look a key up in an empty hashbrown map.
#![cfg_attr(target_arch = "riscv32", no_std, no_main)]
extern crate alloc;
use alloc::vec::Vec;

#[nexus_rt::main]
#[nexus_rt::private_input(input)]
fn main(input: Vec<u8>) -> u32 {
    nexus_rt::println!("input {} bytes", input.len());
    let t = libaw::data::tables();
    nexus_rt::println!("tables: {} units, Infantry costs {}", t.units.len(), t.unit("Infantry").cost);
    input.len() as u32
}
