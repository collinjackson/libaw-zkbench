# zkVM benchmark

Benchmarks of zkVMs running [libaw](https://github.com/collinjackson/libaw)'s referee, for
[libaw-arena](https://github.com/collinjackson/libaw-arena). The arena's current proof design, with per-turn
proofs of the referee and the bot together, is in
[libaw-arena/docs/proofs.md](https://github.com/collinjackson/libaw-arena/blob/main/docs/proofs.md). The section
below is the earlier per-move design that this benchmark started from.

**The goal:** a bot proves that each move came from its own committed program, run on its own
view of the game, with no outside help (no human, no other engine, no hidden network call). The
rules don't need a proof: libaw is reproducible, and a trusted luck source plus the game's action
log let anyone replay a game and check it.

So the program proved is the **bot's decision**: given the seat's fog view (whose hash the server
publishes for every turn) and the bot's committed seed, output an action. The proof shows that a
program with a known image id, committed before the event, maps that view to that action. The
bot's code can stay private: only its image id is public.

### Code, data and inputs are committed separately

- **Code** (public, or at least its image id): the guest program, ideally a shared, audited one
  (libaw plus a standard evaluator or inference runtime) that many bots use. One image id then
  covers every entrant, and verifiers trust code they can read.
- **Data** (private, committed by hash before the event): the bot's weights, tables or settings.
  The proof shows the action came from the data with that hash. A bot can change its data between
  events without a new guest build.
- **Public inputs:** the game's history (actions and revealed luck), from which the guest
  rebuilds the seat's view and any memory the bot keeps. No state is carried between moves outside
  the proof, so there's no side channel for help. In a fog game the full history has hidden
  information, so either proofs are made after the game, or the input is the seat's own view
  history (whose hashes the server publishes as the game goes).

### Setup once, then one step per move

1. **Setup proof** (once per event or game): the guest loads the private data, checks its hash
   against the committed one, and ends with the data in memory. Its public output is the commitment
   and the root of the guest's memory (zkVMs already keep a Merkle root of memory).
2. **Move proofs:** each one resumes from the previous memory root, takes that move's public input
   (history or view), and outputs the action and the new root. The data is never hashed again.
3. **Recursion** folds setup plus every move into one proof per game, which anyone can check
   against the data commitment, the image id and the published history.

A bot that reads all its weights every move still pays for reading them (each zkVM checks the
memory pages a step touches), but that is the zkVM's own accelerated memory check, far cheaper than
hashing the weights in software. If a zkVM can't resume an execution with new input, the fallback
is one proof over the whole game, made after it ends.

### Proofs at turn boundaries, made during the opponent's turn

To keep games quick (Collin, 2026-11-15): a bot plays its turn at full speed with no proof, and the proof
checkpoint is the turn boundary. When it ends its turn, the opponent starts at once, and the bot's turn proof
is made meanwhile.

- **One proof per turn** covers every decision in that turn: it takes the seat's view at the turn start, the
  referee's revealed luck and the legal lists as public input, and outputs the turn's actions. It chains from the
  previous turn's proof (data commitment and memory root), so the setup proof runs once per game.
- **Deadline:** the proof for turn k is due when the bot's next turn would start, with a floor so a quick
  opponent turn can't squeeze it (default: the later of the opponent's turn ending and 120 s after turn k ended).
  No valid proof by then is a resignation. A proof that fails to verify is a forfeit, even if found later.
- **No probing the judge** (Collin, 2026-11-15): a bot must not learn the luck before its action is final, or
  it could try moves against the referee, see the rolls, and change its mind about the turn. So:
  - The referee applies each action as it arrives, and it is final: no take-backs, no dry runs. The turn proof
    must reproduce exactly the actions the referee recorded.
  - The luck seed is secret from both players for the whole game. A public beacon alone isn't enough once bots
    play live over the network (a drand round is public the moment it's out), so the luck key is
    H(referee secret || beacon round): the referee commits to the hash of its secret before the round is drawn
    (so it can't pick luck it likes), keeps the secret during the game, and publishes it after, when anyone can
    check the commitment and replay every roll. A bot sees a roll only after the action that caused it is final.
  - Option that needs no secret at the judge at all (for live bots): each roll is H(beacon, a_i, b_i), where a_i
    and b_i are the two bots' shares for roll i, committed at the start as hash chains. When roll i is needed,
    after the action is final, both reveal share i (checked against their chain). Neither bot can predict a
    roll, since it lacks the other's share; the judge holds nothing to leak; and a bot that doesn't reveal in
    time has resigned, as with a late proof. It costs one round trip per roll.
  - Fog is the one secret the judge must still keep (it sees both sides). Hiding the board from the judge too
    would take mental-poker-style protocols; for open code run on GitHub Actions, trusting the runner is fine.
  - A bot's own randomness comes from a seed it commits to before the game, or from the referee's per-request
    seeds (already keyed by a secret: libaw matchrun), so it can't grind seeds for a lucky line either.
  - The proof's public inputs are the view, the legal lists and the rolls revealed so far; nothing in them
    predicts a future roll.
- **End of game:** the turn proofs fold into one proof for the game (recursion), which is what gets published.
- **What we measure:** proving time and peak memory for one ~20-action turn on a GitHub-hosted runner (4 vCPU, 16 GB) against the deadline.
  That sets how much bot fits: a ranker over the legal list is a few M cycles a decision; a searcher pays for the
  engine inside the proof.

### Keep the rules out of the bot's proof

The rules are public and libaw is deterministic, so anything the referee computes from a seat's
view (the legal actions, damage previews) can be checked by anyone replaying the game and needs no
proof. A bot's proof can then take the legal action list as a public input next to the view, and
cover only the choice. A bot that only ranks the actions it's given stays tiny. A bot that
searches (simulates moves ahead) has to run the engine inside its proof, and then the engine's
cost matters.

What the engine cost in RISC Zero on the old workloads (measured inside the guest): about 3.1M cycles per action
checked (`is_legal`) and applied (`step`); 3.4M once to load the rules tables from JSON; 0.5M to
parse the input; and about 4.8M to serialize and hash the final position (FNV over its JSON).
The output hash is the biggest fixed cost, so a compact encoding before hashing comes before
baking the tables into the binary. The bot workloads will show how that
compares with the bot's own thinking: ranking a given list, a one-step greedy search, and a small
neural net.

## Candidates and the features we need

Wide list first (Collin, 2026-11-15); a zkVM drops out only when it lacks something we need. Must have:
Rust guests that build libaw (`std` or `no_std` + `alloc`), an executor whose output matches native,
continuations (games run to billions of cycles), and **recursion or aggregation down to one proof per
game** that is cheap and easy to verify (a CLI or a library call, no chain needed). **Prover hardware** (Collin, 2026-11-15: example bots for now, so GitHub Actions): a
standard GitHub-hosted runner, which public repos like libaw-arena get free: 4 vCPU, 16 GB RAM, about 14 GB
of disk, no GPU, 6 h per job. A zkVM must prove one turn there, and fold a game's turn proofs into one, within
those limits; peak memory under 16 GB is the hard filter. Turns can be proved in parallel jobs of a matrix once
the game's record exists, then folded in a last job. Nice to have: resuming
from a memory root with new input (one step per move), proving on a Mac or a modest CPU box without a
GPU, a permissive licence.

| zkVM | Guest ISA | Licence | One proof per game (recursion/aggregation) | Round 1 |
|---|---|---|---|---|
| RISC Zero | RV32IM | Apache-2.0 | to check | passes |
| SP1 (Succinct) | RV32IM | MIT/Apache-2.0 | to check | passes |
| Jolt (a16z) | RV64IMAC | MIT | to check (has a recursion example) | passes (move, turn) |
| OpenVM (Axiom) | RV32IM + extensions | MIT/Apache-2.0 | to check | passes |
| ZisK (Polygon) | RV64IMA | Apache-2.0/MIT | to check | |
| Pico (Brevis) | RV64IM | MIT/Apache-2.0 | to check (has an aggregator example) | passes |
| Nexus | RV32IM | BUSL-1.1 until 2029-02-10 (production use barred for a "Competitive Offering"), then MIT/Apache-2.0 | to check | parked |
| Ceno (Scroll) | RV32IM | to check | to check | |
| Airbender (ZKsync) | RV32IM | MIT/Apache-2.0 | to check | |

Licences and ISAs are as listed by each project and get checked against the crates we actually use;
the feature column is filled from each zkVM's docs and a test, not from memory.

## Rounds

Each round drops the zkVMs that fail it, so later, costlier rounds run on fewer:

1. **Builds and matches:** the engine compiles for the zkVM, and the executor's output matches
   native exactly.
2. **Setup then steps:** it can verify the data once, then resume with new input for each move and
   fold the steps into one proof. A zkVM that can only prove a whole game after it ends stays as a
   fallback, but drops out of the main ranking.
3. **Cost:** cycles and proving time for one move's step as the bot gets stronger (search budget)
   and bigger (data size).

What we measure is how much bot fits in a proof: the cost of one decision as the bot gets
stronger (more search), from a one-step greedy bot up.

## The referee program (a building block)

Which zkVM can prove a libaw game, and at what cost? Every zkVM runs the same referee program,
`crates/proof` (`libaw_proof::run`): it reads a position, a list of actions and a luck seed, checks
that each action is legal, applies it, and outputs the winner, the day and an FNV-1a hash of the
final position. It uses no zkVM's built-in hash or other accelerators, so the comparison is fair.

## Workloads

`cargo run --release -p libaw-proof --bin proof-workloads` writes them to `zkvm/workloads/`.
Everything is synthetic: the map is `zkvm/workloads/map.json`, a mirrored 21×19 map written by
`scripts/make_map.py`, and a fixed-seed random player plays both seats from day 1 to day 20 to
reach a mid-game. The workloads start there (33 units on the board).

| Workload | Actions | Input | Native time (M-series Mac, one core) |
|---|---|---|---|
| `empty` | 0 | 9.3 KB | 0.1 ms (loading the rules tables, parsing, hashing) |
| `move` | 1 | 9.3 KB | 0.3 ms |
| `turn` | 18 | 10.6 KB | 1.4 ms |
| `game` | 684 (15 days) | 63.5 KB | 73 ms |

Until 2026-11-15 the workloads started from a position in a real game. The "old workloads" figures in the table below were measured on those and aren't comparable with the new ones.

A zkVM's result counts only if its output matches the native one exactly (`state_hash` included).

These workloads measure the engine's raw cost in each zkVM, since every bot that searches calls
it many times. The main benchmark adds bot workloads (to come): one decision by a greedy bot, and
by a search bot at rising budgets.

## Measures

1. Cycles, from each zkVM's executor (no proof), for the engine workloads and the bot decisions.
2. For one bot decision: proving time, peak memory, proof size and verifying time, one zkVM at a
   time on 6 threads.
3. How strong a bot can be at a given proving cost per decision (search budget vs. win rate).

Results go in the table below as they come in. Run every executor through `scripts/guarded.sh` (kills it above 24 GB): some keep the whole trace in RAM.

| zkVM | Version | License (crates used) | Builds? | Cycles: move / turn / game | Proof: one bot decision |
|---|---|---|---|---|---|
| RISC Zero | 3.0.6 (r0vm) | Apache-2.0 (crates not yet audited) | yes, output matches native | 4.59M / 28.9M / 1.48B user cycles (empty: 0.85M; executor 0.12 s / 0.28 s / 10.6 s on the Mac). Old workloads: 6.84M / 45.5M / 4.88B | |
| SP1 | 6.10.0 (sp1-sdk, blocking CPU executor) | MIT/Apache-2.0 (crates not yet audited) | yes, output matches native | 5.08M / 33.3M / 1.64B cycles (executor 0.09 s / 0.52 s / 13.2 s on the Mac, peak RSS 6.9 / 7.0 / 12.9 GB). Old workloads: 14.2M / 72.1M / 6.39B | |
| Jolt | git 8a92874 (no crates.io release; its arkworks fork and toolchain 1.99.1) | MIT | yes, output matches native | 7.46M / 48.3M / not run (RV64IMAC, so counts aren't directly comparable with RV32IM). Its analyze mode keeps the whole trace in memory (on the old workloads, 3.9 GB peak for a move and 12.9 GB for a turn), so the game workload (billions) can't be traced this way. Old workloads: 20.6M / 106.4M | |
| OpenVM | 2.1.0 (openvm-sdk, metered executor) | MIT/Apache-2.0 (crates not yet audited) | yes, output matches native (length + FNV hash as public values) | 4.53M / 28.6M / 1.46B instructions, within 1.5% of RISC Zero's RV32IM user cycles (metered execute 1.7 s / 1.8 s / 6.5 s, 1.9 GB peak; trace-cell cost 0.16B / 1.04B / 53.0B). Old workloads: 11.75M / 60.9M / 5.73B | |
| Pico | 3.1.3 (pico-sdk/pico-prover git tag, toolchain nightly-2026-09-27) | MIT/Apache-2.0 (crates not yet audited) | yes, output matches native | 4.56M / 29.3M / not rerun (RV64IM; execute 0.55 s / 0.89 s). On the old workloads its chunked emulator was heavy: peak RSS 11 GB / 16 GB / 20.8 GB, so even executing a game needs more than a 16 GB GitHub runner. Old workloads: 11.96M / 62.6M / 5.81B | |
| Nexus | 0.3.6 (last release Jan 2026; guest toolchain nightly-2025-05-09) | BUSL-1.1 (non-production use allowed) | the guest hung: the ELF loader filled `.bss` with the bytes that follow it in the file, fixed in [nexus-xyz/nexus-zkvm#618](https://github.com/nexus-xyz/nexus-zkvm/pull/618) (`zkvm/nexus/repro-min`, `rvref`). With the fix the referee matches native on move and turn. `Emulator::execute` keeps every instruction's records, so memory grows ~1.5 GB/s (an unguarded run reached 159 GB); `zkvm/nexus/host` steps blocks instead | not re-measured: parked for the licence | |

Legality is checked with `libaw::actions::is_legal` (one unit's moves), not the full legal list: that cut the game workload from 10.34B to 5.73B cycles in RISC Zero.

## Turn prover: a bot's turn with its WebAssembly interpreted

`crates/turn` is the turn prover of libaw-arena's proofs design: it takes a position and a bot's
WebAssembly module, runs the bot in wasmi (an interpreter, `no_std`, default features off,
`portable-dispatch`), answers its `libaw.ask` calls natively with libaw-arena's `arena-abi`
crate (the referee's own code), checks and applies each action with libaw, and stops when the
bot ends its turn (cap 400 actions, fuel 2,000,000,000 per call, as in the arena). The RISC Zero
guest (`zkvm/risc0/methods/turn`) commits the module's sha256 and the turn's actions and final
hash; it logs a cycle split from `env::cycle_count`. It needs libaw and libaw-arena checked out
next to this repo.

    cd zkvm/risc0 && cargo build --release -p host --bin turn
    ../../scripts/guarded.sh ./target/release/turn ../workloads/turn.json BOT.wasm

RISC Zero 3.0.6 executor, `turn` workload's start position, libaw-arena's example bots with its
binary interface (bot-abi.md, Nov 2026). Output matches the native run for each.

| Bot | Module | Actions | User cycles | wasmi (bot's own code) | Host answers | View and legal list | Apply actions | Execute (Mac) |
|---|---|---|---|---|---|---|---|---|
| random | 22 KB | 17 | 71M | 43M | 0.8M | 22M | 3.8M | 0.8 s |
| greedy | 23 KB | 18 | 282M | 248M | 0.5M | 28M | 3.8M | 2.3 s |
| hinted (asks for libaw's baseline pick) | 18 KB | 18 | 387M | 3M | 345M | 33M | 3.8M | 3.1 s |
| baseline (libaw inside the module) | 236 KB | 18 | 21.5B | 21.5B | 0.8M | 33M | 3.8M | 164 s |

The native referee's turn is 28.9M. hinted and baseline play the same moves: computing them in
the host costs 345M, and in the interpreter 21.5B, about 60 times more. A bot that brings its
own engine pays for running it interpreted.

With the JSON interface this replaced, where every action the bot got its whole view as JSON and
parsed it inside the interpreter, the same turn cost:

| Bot | Module | Actions | User cycles | wasmi (bot's own code) | libaw queries | Apply actions | Bot input JSON | Load module | Execute (Mac) |
|---|---|---|---|---|---|---|---|---|---|
| greedy | 78 KB | 18 | 2.68B | 2.57B | 50M (18 `legal_actions`) | 31M | 19M | 3.9M | 21 s |
| random | 84 KB | 17 | 2.79B | 2.70B | 44M (17) | 28M | 18M | 4.1M | 22 s |
| baseline (libaw inside the module) | 460 KB | 18 | 27.4B | 27.3B | 0 | 37M | 19M | 21M | 213 s |

Executor peak RSS was under 100 MB. `auto-dispatch` (tail-call dispatch) faults in the guest
with a store access fault; `portable-dispatch` works.
