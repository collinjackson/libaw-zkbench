// A whole game between two bots (crates/turn `play_game`) as a RISC Zero guest. Input on stdin:
// a little-endian u32 length, that many bytes of JSON (libaw_turn::GameInput), a u32 length and
// the first seat's WebAssembly module, then the second seat's module. The journal gets each
// module's sha256 (in seat order), the seed, the result, the number of actions and the final
// state's sha256; the cycle split goes to the log.
use risc0_zkvm::guest::env;
use risc0_zkvm::sha::{Impl, Sha256};
use std::io::Read;

fn main() {
    let mut buf = Vec::new();
    env::stdin().read_to_end(&mut buf).unwrap();
    let n = u32::from_le_bytes(buf[..4].try_into().unwrap()) as usize;
    let input: libaw_turn::GameInput = serde_json::from_slice(&buf[4..4 + n]).unwrap();
    let rest = &buf[4 + n..];
    let m = u32::from_le_bytes(rest[..4].try_into().unwrap()) as usize;
    let (a, b) = (&rest[4..4 + m], &rest[4 + m..]);
    let modules = [Impl::hash_bytes(a).to_string(), Impl::hash_bytes(b).to_string()];
    let (out, end, costs) = libaw_turn::play_game(input, [a, b], env::cycle_count).unwrap();
    let final_sha256 = Impl::hash_bytes(&serde_json::to_vec(&end).unwrap()).to_string();
    env::commit_slice(
        &serde_json::to_vec(&serde_json::json!({"modules_sha256": modules, "final_state_sha256": final_sha256, "game": out})).unwrap(),
    );
    env::log(&format!("costs {}", serde_json::to_string(&costs).unwrap()));
}
