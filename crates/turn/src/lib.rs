//! The turn prover's program (libaw-arena docs/proofs.md): play one turn of an arena bot from a
//! position. The bot is a WebAssembly module (libaw-arena docs/bot-abi.md) run by wasmi, an
//! interpreter, so the proven program is the same whatever the bot. Its `libaw.ask` calls are
//! answered natively by libaw-arena's arena-abi crate, the same code as the referee's, and each
//! action it picks is checked and applied with libaw, as libaw-arena's referee does
//! (crates/arena/src/bot.rs, referee.rs).
//!
//! Luck comes from libaw-proof's `Rolls`, as in the referee workloads, so the cycle counts
//! compare with them.
//!
//! `clock` is a cycle counter (the zkVM's, or a stub natively); `Costs` splits the run by it.
#![no_std]
extern crate alloc;
#[cfg(feature = "std")]
extern crate std;

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use libaw::actions::{is_legal, Action};
use libaw::awbw::action_to_contract;
use libaw::state::State;
use libaw::{fog, sim};
use libaw_proof::{fnv1a, Rolls};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use wasmi::{Caller, Config, Engine, Extern, Instance, Linker, Module, Store, StoreLimits, StoreLimitsBuilder, TypedFunc};

#[derive(Serialize, Deserialize)]
pub struct Input {
    pub state: State,
    pub seed: u64,
    /// After this many actions the turn is ended for the bot (the arena's rule; 400 there).
    pub max_actions: u32,
    /// Fuel for one `arena_pick` call (2,000,000,000 in the arena).
    pub fuel_per_action: u64,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct Move {
    pub action: Value,
    pub fuel: u64,
    pub queries: u32,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub note: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct Output {
    pub seat: u32,
    pub moves: Vec<Move>,
    pub day: u32,
    pub current: u32,
    pub state_hash: u64,
}

/// Cycles by phase. `bot` is the whole of the `arena_act` calls; `query` is the part of it spent
/// answering the bot's questions natively, so `bot - query` is wasmi running the bot.
#[derive(Default, Debug, Serialize, Deserialize)]
pub struct Costs {
    pub load: u64,
    pub input: u64,
    pub bot: u64,
    pub query: u64,
    pub apply: u64,
    pub hash: u64,
    pub queries: u64,
}

struct Ctx {
    view: State,
    legal: Vec<Action>,
    limits: StoreLimits,
    queries: u32,
    query_cycles: u64,
    clock: fn() -> u64,
}

fn pack(ptr: u32, len: u32) -> i64 {
    ((ptr as u64) << 32 | len as u64) as i64
}

struct Bot {
    module: Module,
    linker: Linker<Ctx>,
    engine: Engine,
    fuel: u64,
    clock: fn() -> u64,
    live: Option<(Store<Ctx>, Instance)>,
}

const MEMORY: usize = 64 << 20;

impl Bot {
    fn new(wasm: &[u8], fuel: u64, clock: fn() -> u64) -> Result<Bot, String> {
        let mut cfg = Config::default();
        cfg.consume_fuel(true);
        let engine = Engine::new(&cfg);
        let module = Module::new(&engine, wasm).map_err(|e| format!("not a wasm module: {e}"))?;
        for imp in module.imports() {
            if (imp.module(), imp.name()) != ("libaw", "ask") {
                return Err(format!("imports {}.{}: bots may import only libaw.ask", imp.module(), imp.name()));
            }
        }
        let mut linker = Linker::new(&engine);
        linker
            .func_wrap("libaw", "ask", |mut caller: Caller<'_, Ctx>, kind: i32, arg: i32| -> Result<i64, wasmi::Error> {
                let t0 = (caller.data().clock)();
                let out = arena_abi::answer(&caller.data().view, &caller.data().legal, kind, arg);
                let t1 = (caller.data().clock)();
                caller.data_mut().queries += 1;
                caller.data_mut().query_cycles += t1 - t0;
                let mem = caller.get_export("memory").and_then(Extern::into_memory).ok_or_else(|| wasmi::Error::new("no memory export"))?;
                let alloc = caller.get_export("arena_alloc").and_then(Extern::into_func).ok_or_else(|| wasmi::Error::new("no arena_alloc export"))?;
                let at = alloc.typed::<i32, i32>(&caller)?.call(&mut caller, out.len() as i32)?;
                mem.write(&mut caller, at as u32 as usize, &out).map_err(|e| wasmi::Error::new(e.to_string()))?;
                Ok(pack(at as u32, out.len() as u32))
            })
            .map_err(|e| e.to_string())?;
        Ok(Bot { module, linker, engine, fuel, clock, live: None })
    }

    fn instance(&mut self, view: State, legal: Vec<Action>) -> Result<(), String> {
        if let Some((store, _)) = &mut self.live {
            store.data_mut().view = view;
            store.data_mut().legal = legal;
            store.data_mut().queries = 0;
            return Ok(());
        }
        let limits = StoreLimitsBuilder::new().memory_size(MEMORY).instances(1).tables(4).memories(1).build();
        let mut store = Store::new(&self.engine, Ctx { view, legal, limits, queries: 0, query_cycles: 0, clock: self.clock });
        store.limiter(|c| &mut c.limits);
        store.set_fuel(self.fuel).map_err(|e| e.to_string())?;
        let inst = self.linker.instantiate_and_start(&mut store, &self.module).map_err(|e| format!("instantiate: {e}"))?;
        self.live = Some((store, inst));
        Ok(())
    }

    /// One decision; returns the action picked (None for a pick outside the legal list), or why
    /// there is none, fuel used, queries asked and the cycles spent answering them.
    fn act(&mut self, view: State, legal: Vec<Action>) -> (Result<Option<Action>, String>, u64, u32, u64) {
        let seat = view.current;
        if let Err(e) = self.instance(view, legal) {
            self.live = None;
            return (Err(e), 0, 0, 0);
        }
        let (store, inst) = self.live.as_mut().unwrap();
        store.data_mut().query_cycles = 0;
        let _ = store.set_fuel(self.fuel);
        let res = (|| -> Result<Option<Action>, wasmi::Error> {
            let pick: TypedFunc<i32, i32> = inst.get_typed_func(&mut *store, "arena_pick")?;
            let i = pick.call(&mut *store, seat as i32)?;
            Ok(arena_abi::resolve(&store.data().legal, i))
        })();
        let left = store.get_fuel().unwrap_or(0);
        let (queries, qc) = (store.data().queries, store.data().query_cycles);
        let used = self.fuel - left;
        let res = res.map_err(|e| {
            let why = if left == 0 || e.as_trap_code() == Some(wasmi::TrapCode::OutOfFuel) {
                "out of fuel".to_string()
            } else {
                format!("fault: {e}").chars().take(300).collect()
            };
            self.live = None; // a trapped instance starts fresh next time
            why
        });
        (res, used, queries, qc)
    }
}

fn legal_for_mover(s: &State, a: &Action) -> bool {
    if s.fog {
        is_legal(&fog::player_view(s, s.current), a)
    } else {
        is_legal(s, a)
    }
}

/// Plays the seat to move's turn: asks the bot for an action until it ends its turn (an illegal
/// action, a fault or the action cap ends it for the bot), applying each with libaw.
pub fn play_turn(input: Input, wasm: &[u8], clock: fn() -> u64) -> Result<(Output, Costs), String> {
    let mut c = Costs::default();
    let t = clock();
    let mut bot = Bot::new(wasm, input.fuel_per_action, clock)?;
    c.load = clock() - t;
    let Input { state: mut s, seed, max_actions, .. } = input;
    let mut rolls = Rolls::new(seed);
    let me = s.current;
    let mut moves = vec![];
    loop {
        if sim::winner(&s).is_some() {
            break;
        }
        let t0 = clock();
        let seen = fog::player_view(&s, me);
        let legal = arena_abi::legal(&seen);
        let t1 = clock();
        let (reply, fuel, queries, qc) = bot.act(seen, legal);
        let t2 = clock();
        let (a, mut note) = match reply {
            Ok(Some(a)) if legal_for_mover(&s, &a) => (a, None),
            Ok(_) => (Action::EndTurn, Some("bad pick: not -1 or an index into the legal list".to_string())),
            Err(e) => (Action::EndTurn, Some(e)),
        };
        let a = if moves.len() as u32 >= max_actions && a != Action::EndTurn {
            note = Some("action limit for the turn".to_string());
            Action::EndTurn
        } else {
            a
        };
        sim::step(&mut s, &a, &mut |lo, hi| rolls.roll(lo, hi)).map_err(|e| format!("engine refused an action: {e}"))?;
        let t3 = clock();
        c.input += t1 - t0;
        c.bot += t2 - t1;
        c.query += qc;
        c.queries += queries as u64;
        c.apply += t3 - t2;
        let end = a == Action::EndTurn;
        moves.push(Move { action: action_to_contract(&a), fuel, queries, note });
        if end {
            break;
        }
    }
    let t = clock();
    let state_hash = fnv1a(&serde_json::to_vec(&s).unwrap());
    c.hash = clock() - t;
    Ok((Output { seat: me as u32, moves, day: s.day as u32, current: s.current as u32, state_hash }, c))
}

