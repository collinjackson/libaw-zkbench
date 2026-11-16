// The turn prover (crates/turn) as a RISC Zero guest. Input on stdin: a little-endian u32 length,
// that many bytes of JSON (libaw_turn::Input), then the bot's WebAssembly module. The journal
// gets the module's sha256 and the turn (its actions and the final state's hash); the cycle split
// goes to the log.
use risc0_zkvm::guest::env;
use risc0_zkvm::sha::{Impl, Sha256};
use std::io::Read;

fn main() {
    let mut buf = Vec::new();
    env::stdin().read_to_end(&mut buf).unwrap();
    let n = u32::from_le_bytes(buf[..4].try_into().unwrap()) as usize;
    let t = env::cycle_count();
    let input: libaw_turn::Input = serde_json::from_slice(&buf[4..4 + n]).unwrap();
    let parse = env::cycle_count() - t;
    let wasm = &buf[4 + n..];
    let t = env::cycle_count();
    let hash = Impl::hash_bytes(wasm);
    let sha = env::cycle_count() - t;
    let (out, costs) = libaw_turn::play_turn(input, wasm, env::cycle_count).unwrap();
    env::commit_slice(&serde_json::to_vec(&serde_json::json!({"module_sha256": hash.to_string(), "turn": out})).unwrap());
    env::log(&format!("costs {} parse {parse} sha256 {sha}", serde_json::to_string(&costs).unwrap()));
}
