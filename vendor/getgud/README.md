# getgud

A small, dependency-free **rollback netcode** core for deterministic games with
one local player and any number of remote peers, in Rust.

It handles the hard part of peer-to-peer netcode: confirming the input rows every
peer agrees on, predicting the remote inputs that haven't arrived yet, correcting
those predictions once the real inputs land, and producing a clock-sync signal so
the peers keep their simulations aligned. The crate contains no game logic —
you bring the simulation.

## API

You implement one trait — `World` — on the type that owns your live simulation,
then drive a `Session` once per tick. Newly settled input rows come back from
`Session::advance` beside the frame to present.

### `World`

A `World` is a *live* simulation parked at one tick, which the session can step,
snapshot, and restore. It names the three types the crate is generic over
(`Input`, `State`, `Error`) and supplies four methods, plus two optional hooks:

| Member                            | Responsibility                                                                                       |
|-----------------------------------|------------------------------------------------------------------------------------------------------|
| `Input` / `State` / `Error`       | Your per-tick input, snapshot, and error types. Use `Infallible` for `Error` if stepping can't fail. |
| `step(&local, &remotes)`          | Step the live simulation one tick from where it's parked, applying the input row (`remotes` indexed by remote slot). **Must be deterministic.** |
| `save() -> State`                 | Snapshot the live simulation at the tick it's parked at. The session keeps snapshots to present, to promote a correct prediction without re-simulating, and to reload on rollback. |
| `load(&State)`                    | Restore the live simulation to a snapshot, parking it at that tick. Called to rewind before re-simulating a mispredicted tail. The session tracks where the world is parked and never loads the snapshot of that tick (unless the world's position is unknown: after `world_mut` or an error). |
| `predict(&last_remote) -> Input`  | Guess a remote's next input from its last confirmed one (e.g. "repeat the last input"). Applied per remote slot, only where the real input hasn't arrived. |
| `recycle(State)` (optional)       | Take back a snapshot the session discards, to reuse its allocation. |
| `settled(&Confirmed)` (optional)  | Told about each input row as it joins the settled state, in tick order, once each, after the world's last step of it: for what the world keeps, such as an observer of the ticks it simulates. |

`step` doesn't return a snapshot: the snapshot comes from `save`, called only
when the session actually needs to keep that tick. That split lets a rollback
re-`step` through a corrected tail and `save` just its final state, instead of
snapshotting every intermediate tick.

`Input` must be `Clone + PartialEq` (predictions are compared against the real
inputs to decide promote-vs-rollback); `Input`, `State` and `Error` must be
`Send + 'static`. A session is `Send` when its world is, so it can run on a
thread of its own.

Determinism is the one hard requirement: the same snapshot loaded and stepped by
the same input row must always produce the same next state. Rollback depends on
it.

## Operation

Each peer constructs one `Session` from
`SessionParams { present_delay, initial_remotes, initial_state, world }` (the
length of `initial_remotes` fixes the remote count; the world must be parked at
`initial_state`). The session runs for as long as the host drives it. The
per-tick loop:

```text
loop each tick:
    while a remote packet arrived:
        session.add_remote_input(slot, remote_input, their_tick_advantage)
    adjust_clock(session.skew());        // stall a frame when running ahead
    let advanced = session.advance(local_input)?;
    record(advanced.confirmed);          // zero or more newly settled rows
    render(advanced.frame.state);        // also: frame.tick, frame.local, frame.remotes
```

`advance` enqueues the local input, confirms every tick all peers now agree on,
advances the displayed state, and returns an
`Advance { frame: Frame { tick, state, local, remotes }, confirmed }`.
`confirmed` lists the rows that joined the authoritative settled state, exactly
once and in tick order: each `Confirmed { tick, local, remotes, settlement,
state }` says its tick, whether its speculation was promoted or it was simulated
as it settled, and the settled state after it where the session kept one (a
promoted row's snapshot, and the last row a call simulated).
The clock-sync hint is read *separately* via `session.skew()` — and read it
*before* `advance`, which enqueues this tick's local input.

Between calls the host reaches the world with `session.world()` (and
`world_mut()`, after which the session reloads its own snapshot before it next
steps the world), and the settled state with `session.settled_state()`;
`session.into_world()` hands the world back when the session is done (to carry
what it keeps into the next round's session).

**Ending a session.** Once the local input has ended (a replay's last tick, or
the tick the peers agreed a match ends on), call `session.drain()` once a tick
instead of `advance`: it settles every row the remotes' last inputs confirm,
however far past the present target, without speculating past the last input,
and returns the rows. `session.is_drained()` says when every local input has
settled; then tear the session down. A match that just stops (the game is over
in the settled state) needs no draining.

**Errors.** A `World` error comes back from `advance` (or `drain`), and the
session stays consistent: the local input is taken, rows settled before the
error come back from the next call that succeeds, and the next call retries
what failed, reloading one of its own snapshots first.

### Key terms

- **Local frontier** — the newest local tick; `advance` bumps it by one.
- **Present delay** — how many ticks behind the frontier you display
  (`target = frontier − present_delay`). Larger = less prediction but more input
  latency; smaller = snappier but speculates further. Tunable at runtime via
  `set_present_delay`.
- **Settled state** — the authoritative state, folded only from confirmed input
  rows (every player's input present). `advance` returns rows in tick order as
  they settle.
- **Speculation** — when the display target runs past the last confirmed tick, the
  session steps forward from the settled state with real local inputs and, per
  remote slot, the real input where it has already arrived (a tick can be
  unconfirmed because a *different* remote is still missing) or a
  `predict`-supplied one where it hasn't, `save`-ing each speculative snapshot
  into a rolling buffer.
- **Promote vs. rollback** — when the outstanding real inputs arrive, the session
  compares them against what it speculated with. The matching prefix is *promoted*
  to settled with no re-simulation (those snapshots are already byte-exact); only
  from the first misprediction on does it `load` and re-`step` the corrected
  inputs. Each returned row says which happened to it (`Settlement::Promoted` or
  `Simulated`); `last_misprediction_depth()` reports how deep the last rollback
  went.
- **Skew** — clock sync. Each peer reports how far its local input leads the
  furthest-behind remote input it has received (`local_tick_advantage`), ships
  that to every peer alongside each input, and gets each peer's value back through
  `add_remote_input`. `skew()` is the worst case over the remotes of the
  difference; positive means you're ahead of someone — stall a frame to converge.

## Prediction vs. delay

Whether the session speculates depends on how far confirmed input has progressed
relative to the displayed tick. Both diagrams share `frontier` 9 and
`present_delay` 3 (so `target` 6), differing only in how much remote input has
arrived.

**Prediction regime** — confirmed input lags the present, so `target` sits past
the last confirmed tick and the session speculates the gap:

```text
 tick  0   1   2   3   4   5   6   7   8   9
       ●───●───●───●───○───○───○───◌───◌───◌
                   │           │           │
                   │           │           └─ frontier (newest local tick)
                   │           └─ target = frontier − present_delay
                   │                (the frame you render)
                   └─ last tick confirmed by every peer (settled)

   ●  confirmed  — real local + real remotes, folded into settled state
   ○  speculated — real local + arrived-or-predicted remotes, simulated up to
                   the target (later promoted if the predictions held, else
                   rolled back)
   ◌  buffered   — local input entered, still past the target, not yet simulated
```

**Delay regime** — confirmed input has caught up, so `target` is at or behind the
last confirmed tick and the rendered frame is already confirmed; no prediction
runs. A large enough `present_delay` (or low latency) keeps you here:

```text
 tick  0   1   2   3   4   5   6   7   8   9
       ●───●───●───●───●───●───●───●───◌───◌
                               │   │       │
                               │   │       └─ frontier (newest local tick)
                               │   └─ last confirmed tick
                               └─ target — confirmed frame you render

   ●  confirmed — real local + real remotes (settled state)
   ◌  buffered  — local input entered, awaiting a remote
```
