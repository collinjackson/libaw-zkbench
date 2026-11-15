// The libaw referee program (crates/proof) as a Nexus guest. Nexus guests have no std, so the
// engine and the referee build with default-features = false (alloc only). Input JSON in as the
// private input, output JSON out as the public output.
#![cfg_attr(target_arch = "riscv32", no_std, no_main)]
extern crate alloc;
use alloc::vec::Vec;

#[nexus_rt::main]
#[nexus_rt::private_input(input)]
fn main(input: Vec<u8>) -> Vec<u8> {
    let out = libaw_proof::run(serde_json::from_slice(&input).unwrap());
    serde_json::to_vec(&out).unwrap()
}
