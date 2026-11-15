use libaw_proof::{run, Input};

fn load(name: &str) -> Input {
    let p = format!("{}/../../zkvm/workloads/{name}.json", env!("CARGO_MANIFEST_DIR"));
    serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()
}

#[test]
fn the_turn_workload_gives_the_same_result_every_run() {
    let a = run(load("turn")).unwrap();
    let b = run(load("turn")).unwrap();
    assert_eq!(a, b);
    assert_eq!((a.actions, a.day), (17, 22));
}

#[test]
fn an_illegal_action_is_refused_with_its_index() {
    let mut input = load("turn");
    let first = input.actions[0].clone();
    input.actions.insert(1, first); // the same unit can't act twice
    let e = run(input).unwrap_err();
    assert!(e.starts_with("action 1:"), "{e}");
}

#[test]
fn a_different_seed_changes_the_luck_and_so_the_result() {
    let mut input = load("game");
    let base = run(load("game")).unwrap();
    input.seed ^= 1;
    // Other rolls can make a later action illegal (a unit that should have died), or just change HP.
    match run(input) {
        Ok(o) => assert_ne!(o.state_hash, base.state_hash),
        Err(e) => assert!(e.starts_with("action "), "{e}"),
    }
}
