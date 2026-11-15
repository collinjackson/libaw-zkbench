//! Writes the zkVM benchmark's three workloads and checks them natively. Everything is
//! synthetic: the map is zkvm/workloads/map.json (scripts/make_map.py), and a fixed-seed random
//! player that prefers acting to ending its turn plays both seats from day 1 to day 20 to
//! reach a mid-game position. From there:
//!   empty: no actions (the fixed costs);
//!   move: one action;
//!   turn: that player's whole turn;
//!   game: 15 days of play (both players).
//! The files are the same on every run.
//!
//!   cargo run --release -p libaw-proof --bin proof-workloads -- OUT_DIR
use libaw::actions::{legal_actions, Action};
use libaw::sim::step;
use libaw_proof::{run, Input, Rolls};
use std::time::Instant;

const MAP: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../zkvm/workloads/map.json");
const SEED: u64 = 22;
const WARMUP_DAYS: u32 = 20;
const MAX_ACTIONS_PER_TURN: usize = 40;

/// The random player: a random legal action other than ending the turn, until it has none or
/// has taken MAX_ACTIONS_PER_TURN this turn.
fn pick_action(s: &libaw::State, this_turn: usize, pick: &mut Rolls) -> Action {
    let legal: Vec<Action> = legal_actions(s).into_iter().filter(|a| *a != Action::EndTurn).collect();
    if legal.is_empty() || this_turn >= MAX_ACTIONS_PER_TURN {
        Action::EndTurn
    } else {
        legal[pick.roll(0, legal.len() as i32 - 1) as usize].clone()
    }
}

fn main() {
    let out = std::env::args().nth(1).unwrap_or("zkvm/workloads".into());
    std::fs::create_dir_all(&out).unwrap();
    let map: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(MAP).unwrap()).unwrap();
    let mut s = libaw::awbw::state_from(&map);
    let (mut rolls, mut pick) = (Rolls::new(SEED), Rolls::new(SEED ^ 0xa5a5));
    let mut this_turn = 0;
    while s.day < WARMUP_DAYS {
        assert!(libaw::sim::winner(&s).is_none(), "warm-up game ended on day {}", s.day);
        let a = pick_action(&s, this_turn, &mut pick);
        this_turn = if a == Action::EndTurn { 0 } else { this_turn + 1 };
        step(&mut s, &a, &mut |lo, hi| rolls.roll(lo, hi)).unwrap();
    }
    let start = s.clone();
    let (day0, who0) = (start.day, start.current);
    println!("start: day {day0}, {} units, funds {:?}", start.units.len(), start.players.iter().map(|p| p.funds).collect::<Vec<_>>());
    let mut actions = vec![];
    let (mut move_n, mut turn_n) = (None, None);
    this_turn = 0;
    while libaw::sim::winner(&s).is_none() && s.day < day0 + 15 {
        let a = pick_action(&s, this_turn, &mut pick);
        actions.push(libaw::awbw::action_to_contract(&a));
        let end = a == Action::EndTurn;
        step(&mut s, &a, &mut |lo, hi| rolls.roll(lo, hi)).unwrap();
        move_n.get_or_insert(actions.len());
        if end {
            this_turn = 0;
            if turn_n.is_none() && (s.current != who0 || s.day != day0) {
                turn_n = Some(actions.len());
            }
        } else {
            this_turn += 1;
        }
    }

    for (name, n) in [("empty", 0), ("move", move_n.unwrap()), ("turn", turn_n.unwrap()), ("game", actions.len())] {
        let input = Input { state: start.clone(), actions: actions[..n].to_vec(), seed: SEED };
        let json = serde_json::to_string(&input).unwrap();
        std::fs::write(format!("{out}/{name}.json"), &json).unwrap();
        let t = Instant::now();
        let o = run(serde_json::from_str(&json).unwrap()).unwrap();
        println!("{name}: {} actions, {} bytes, native {:.1} ms, {:?}", n, json.len(), t.elapsed().as_secs_f64() * 1e3, o);
    }
}
