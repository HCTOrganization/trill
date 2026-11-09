use std::collections::VecDeque;

use crate::input::Queue;
use crate::world::World;

/// Everything needed to construct a [`Session`].
///
/// Pass this to [`Session::new`].
pub struct SessionParams<W: World> {
    /// How many ticks behind the local frontier to present.
    ///
    /// A larger value shows older, more-often-confirmed state (less prediction,
    /// more input latency); a smaller value is more responsive but speculates
    /// further ahead. This is the classic input-delay knob and can be changed at
    /// runtime via [`Session::set_present_delay`].
    pub present_delay: u32,

    /// One entry per remote peer: the input assumed for that peer at tick 0,
    /// before any real input has arrived. Used as the seed for prediction.
    /// The length fixes the session's remote count (at least one).
    pub initial_remotes: Vec<W::Input>,

    /// The starting game state at tick 0.
    pub initial_state: W::State,

    /// The game-specific world that advances the simulation, parked at
    /// `initial_state` (the session steps it from there without loading
    /// anything first). See [`World`].
    pub world: W,
}

/// The result of one [`Session::advance`] call: the state to present this
/// frame, plus the metadata needed to render it. The clock-sync hint is read
/// separately via [`Session::skew`].
pub struct Frame<'a, W: World> {
    /// The tick this frame represents (`frontier - present_delay`, clamped to
    /// what has been simulated). May be a speculative tick or a fully confirmed
    /// one depending on how far remote input has lagged.
    pub tick: u32,

    /// The game state to render this frame, borrowed from the session. It may be
    /// the authoritative settled state or a speculative one built from predicted
    /// remote input.
    pub state: &'a W::State,

    /// The local input that applies at [`tick`](Frame::tick) — handy for
    /// presentation that should line up with the frame being shown (local
    /// audio, effects, UI).
    pub local: W::Input,

    /// The remote inputs that apply at [`tick`](Frame::tick), indexed by
    /// remote slot. For a confirmed tick these are the real inputs received
    /// from the peers; for a speculative tick, each slot holds the real input
    /// where one had arrived and the [`predict`](World::predict)-supplied
    /// guess where it hadn't.
    pub remotes: Box<[W::Input]>,
}

/// Everything produced by one [`Session::advance`] call: the frame to present
/// and every input row that became part of the authoritative settled state.
pub struct Advance<'a, W: World> {
    pub frame: Frame<'a, W>,
    /// The rows that joined the settled state, in tick order. Each row is
    /// returned exactly once.
    pub confirmed: Vec<Confirmed<'a, W>>,
}

/// One input row that joined the settled state: every player's input for
/// one tick, and how the session settled it.
pub struct Confirmed<'a, W: World> {
    /// The row's tick: its inputs take the settled state from `tick` to
    /// `tick + 1`. Rows are numbered from 0, with no gaps.
    pub tick: u32,
    /// The local player's input.
    pub local: W::Input,
    /// The remote inputs, in the same remote-slot order as
    /// [`Session::add_remote_input`].
    pub remotes: Box<[W::Input]>,
    /// Whether the row's speculation was promoted or the row was simulated
    /// as it settled.
    pub settlement: Settlement,
    /// The settled state after the row (at `tick + 1`), where the session
    /// kept one: a promoted row's speculative snapshot, and the last of the
    /// rows a call simulated, which the session saved as its new settled
    /// state. `None` for the rows simulated before that one, which were
    /// stepped without a save.
    pub state: Option<&'a W::State>,
}

/// How a confirmed row joined the settled state.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Settlement {
    /// Its speculation had the real inputs (every prediction for it held,
    /// and for every row before it), so its snapshot became the settled
    /// state as it was: nothing was simulated.
    Promoted,
    /// It was simulated on its real inputs as it settled: again, when its
    /// speculation was thrown away for a wrong prediction (here or in an
    /// earlier row), or for the first time, when it was never speculated
    /// (the present target hadn't reached it).
    Simulated,
}

/// A row that joined the settled state and hasn't been returned yet.
struct SettledRow<I> {
    tick: u32,
    local: I,
    remotes: Box<[I]>,
    settlement: Settlement,
    /// The session kept the state after it: the settled state, while the
    /// row is the newest settled one, and then a state in `trail`.
    has_state: bool,
}

/// One speculatively-simulated tick, kept in a rolling buffer so a correct
/// prediction can be promoted to settled with no re-simulation, and a wrong one
/// can be rolled back to.
struct Speculation<W: World> {
    /// The tick this snapshot is poised at (one past the tick its inputs
    /// advanced). `speculations[i].tick == settled_tick + 1 + i`.
    tick: u32,
    /// The bundled game state at the start of [`tick`](Self::tick).
    state: W::State,
    /// The real local input that produced this snapshot.
    local: W::Input,
    /// The remote inputs that produced this snapshot, per slot: real where
    /// the input had already arrived, predicted where it hadn't. This is what
    /// a later confirmation is checked against to decide promote-vs-rollback.
    remotes: Box<[W::Input]>,
}

/// A single peer's view of a rollback session: one local player against any
/// number of remote peers (a classic two-player session has exactly one).
///
/// `Session` owns the local/remote input queues, the authoritative ("settled")
/// state, and a rolling buffer of speculative snapshots shown to the player. The
/// intended loop is:
///
/// 1. As remote packets arrive, call [`add_remote_input`](Session::add_remote_input)
///    with the sending peer's slot.
/// 2. Once per tick, read [`skew`](Session::skew) and feed it into your clock so
///    the peers stay aligned, then call [`advance`](Session::advance) with
///    the local input. `advance` confirms any newly-matched ticks into the
///    settled state — promoting the speculative snapshots whose prediction held
///    and rolling back only where it didn't — extends speculation forward as
///    needed, and returns a [`Frame`] to render.
///
/// A tick is *confirmed* only once every remote's input for it has arrived;
/// until then it is speculative. Speculation uses the real input for any
/// remote whose packet is already in (only the missing slots are predicted),
/// so a fast peer's inputs never get second-guessed while waiting on a slow
/// one.
///
/// Construct one with [`Session::new`]. See the [crate-level docs](crate) for a
/// complete example.
///
/// A session is [`Send`] when its world is (its inputs and states always are),
/// so it can run on a thread of its own, a network thread for one.
pub struct Session<W: World> {
    present_delay: u32,

    world: W,

    local_frontier: u32,

    input_queue: Queue<W::Input>,

    /// Rows that joined the settled state and haven't been returned yet, in
    /// tick order: those of the current call, and those of a call that
    /// failed after settling them.
    settled_rows: Vec<SettledRow<W::Input>>,
    /// The states after `settled_rows` that have one, in order, except the
    /// newest row's (the settled state). Kept so the returned rows can lend
    /// them, and recycled at the start of the call after.
    trail: Vec<W::State>,

    /// Number of confirmed input rows ever matched (cumulative).
    /// Confirmed ticks are `0..confirm_frontier`. Invariant:
    /// `confirm_frontier == settled_tick + settle_backlog.len()`.
    confirm_frontier: u32,
    /// Per remote slot, the last input confirmed for that peer — the base of
    /// its prediction chain.
    last_confirmed_remotes: Box<[W::Input]>,

    /// Confirmed `(local, remotes)` rows not yet folded into the settled
    /// state, in tick order (covering ticks `settled_tick..confirm_frontier`).
    /// They are held back until the present target reaches them so the
    /// settled state never runs past the frame being displayed.
    settle_backlog: VecDeque<(W::Input, Box<[W::Input]>)>,
    /// The authoritative state, built only from confirmed-and-correct rows.
    settled_state: W::State,
    /// The tick `settled_state` is poised at (number of rows folded in).
    settled_tick: u32,
    /// Rolling buffer of speculative snapshots covering `(settled_tick, …]`,
    /// contiguous and in tick order.
    speculations: VecDeque<Speculation<W>>,
    /// The tick the world is parked at, when the session knows it: the
    /// settled state's or a speculation's, whose snapshot the session
    /// holds. The session loads a snapshot only to move the world, never
    /// the one it is parked at. `None` once the world may be anywhere,
    /// which makes the next use load a snapshot first.
    parked: Option<u32>,

    /// How many speculative frames the most recent [`advance`](Session::advance)
    /// discarded and re-simulated because a confirmed remote input didn't match
    /// the prediction — i.e. the rollback depth for that frame, surfaced as a
    /// telemetry signal via [`misprediction_depth`](Session::misprediction_depth).
    /// 0 when the frame promoted cleanly or didn't settle.
    last_misprediction_depth: u32,

    /// Per remote slot, the tick advantage that peer last reported.
    last_remote_tick_advantages: Box<[i16]>,
}

impl<W: World> Session<W> {
    /// Create a session from the given [`SessionParams`], seeded at tick 0.
    pub fn new(params: SessionParams<W>) -> Self {
        let SessionParams {
            present_delay,
            initial_remotes,
            initial_state,
            world,
        } = params;
        assert!(!initial_remotes.is_empty(), "a session needs at least one remote");

        Self {
            present_delay,
            world,
            local_frontier: 0,
            input_queue: Queue::new(initial_remotes.len()),
            settled_rows: Vec::new(),
            trail: Vec::new(),
            confirm_frontier: 0,
            last_remote_tick_advantages: initial_remotes.iter().map(|_| 0).collect(),
            last_confirmed_remotes: initial_remotes.into_boxed_slice(),
            settle_backlog: VecDeque::new(),
            settled_state: initial_state,
            settled_tick: 0,
            speculations: VecDeque::new(),
            parked: Some(0),
            last_misprediction_depth: 0,
        }
    }

    /// Number of remote peers in this session.
    pub fn num_remotes(&self) -> usize {
        self.input_queue.num_remotes()
    }

    /// The local frontier: the number of ticks [`advance`](Session::advance) has
    /// been called, i.e. the newest local tick.
    pub fn local_frontier(&self) -> u32 {
        self.local_frontier
    }

    /// The current present delay — how many ticks behind the frontier frames are
    /// presented. See [`SessionParams::present_delay`].
    pub fn present_delay(&self) -> u32 {
        self.present_delay
    }

    /// Change the present delay at runtime. Takes effect on the next
    /// [`advance`](Session::advance).
    pub fn set_present_delay(&mut self, present_delay: u32) {
        self.present_delay = present_delay;
    }

    /// The tick of the settled state: how many input rows it has folded in.
    pub fn settled_tick(&self) -> u32 {
        self.settled_tick
    }

    /// Whether every local input given so far has settled: the end of a
    /// session that [`drain`](Session::drain) is ending.
    pub fn is_drained(&self) -> bool {
        self.settled_tick == self.local_frontier
    }

    /// The authoritative settled state. Exposed so the host can read host-side
    /// data bundled into the state (e.g. to re-anchor an auxiliary simulator at
    /// round end). The engine treats it as opaque.
    pub fn settled_state(&self) -> &W::State {
        &self.settled_state
    }

    /// The world the session drives: for what it keeps besides the live
    /// simulation (an observer of the ticks it simulates, say), which the
    /// host reads between advances.
    pub fn world(&self) -> &W {
        &self.world
    }

    /// The world, to change. Anything goes, the live simulation included:
    /// the session no longer knows where the world is parked, and loads a
    /// snapshot of its own before it next steps it (one
    /// [`load`](World::load) more than it would otherwise need). For reads,
    /// [`world`](Session::world) costs nothing.
    pub fn world_mut(&mut self) -> &mut W {
        self.parked = None;
        &mut self.world
    }

    /// End the session and take its world back: to carry what it keeps (an
    /// observer, say) into the next session, a match's next round. Its live
    /// simulation is wherever the session left it; the authoritative state is
    /// the [`settled_state`](Session::settled_state), to read before.
    pub fn into_world(self) -> W {
        self.world
    }

    /// Advance the simulation by one local tick and return the frame to present
    /// plus every input row settled by this call.
    ///
    /// This is the per-tick driver. It:
    ///
    /// 1. enqueues `local_input`;
    /// 2. matches it against the buffered remote inputs and folds the newly
    ///    confirmed ticks (those with every remote's input present) into the
    ///    authoritative settled state — promoting the speculative snapshots
    ///    whose predicted remotes all matched (no re-sim) and rolling back to
    ///    re-simulate where they didn't, collecting the newly settled rows and
    ///    telling the world about each ([`World::settled`]);
    /// 3. computes the present target (`frontier - present_delay`);
    /// 4. extends the speculative buffer forward to the target — using each
    ///    remote's real input where it has already arrived and a
    ///    [`predict`](World::predict)-supplied guess where it hasn't — reusing
    ///    the snapshots already built (only the genuinely new ticks are
    ///    simulated);
    /// 5. returns the state, tick, and input row at that tick, together with the
    ///    confirmed rows from step 2.
    ///    (Read [`skew`](Session::skew) *before* this call for the clock-sync
    ///    hint covering the tick being advanced.)
    ///
    /// # Errors
    ///
    /// Propagates any [`W::Error`](World::Error) returned by the
    /// [`World`](crate::World). The session stays consistent: `local_input` is
    /// taken all the same, the rows settled before the error stay settled and
    /// come back from the next call that succeeds, and the next call retries
    /// what failed, loading one of its own snapshots first since the world may
    /// have been left anywhere.
    pub fn advance(&mut self, local_input: W::Input) -> Result<Advance<'_, W>, W::Error> {
        self.begin_call();
        // The new input's tick is the frontier; present `present_delay` behind it.
        let target = self.local_frontier.saturating_sub(self.present_delay);
        self.input_queue.add_local_input(local_input);
        self.local_frontier += 1;
        let (unmatched_locals, unmatched_remotes) = self.match_inputs();

        // Fold confirmed rows into the settled state, but never past the present
        // target (so the settled state stays at or behind the frame we display).
        self.settle_to(target.min(self.confirm_frontier))?;

        // Extend the speculative tail up to the present target, using arrived
        // remote inputs where available and predictions elsewhere, reusing the
        // snapshots already built.
        let speculating = target > self.settled_tick && self.confirm_frontier > 0;
        if speculating {
            self.speculate_to(target, &unmatched_locals, &unmatched_remotes)?;
        }

        let rows = std::mem::take(&mut self.settled_rows);
        let confirmed = lend(rows, &self.trail, &self.settled_state);

        // Present the speculation at `target`. The buffer can reach *past* `target`
        // when `present_delay` was just increased (shrinking `target`) since those
        // deeper ticks were speculated, so index to `target` rather than taking the
        // deepest snapshot — the surplus tail stays buffered for the frames where
        // `target` climbs back to it (or is dropped wholesale on the next
        // rollback). If there is no speculation, present the settled state.
        let frame = if speculating {
            let spec = &self.speculations[(target - self.settled_tick - 1) as usize];
            assert_eq!(spec.tick, target);
            Frame {
                tick: spec.tick,
                state: &spec.state,
                local: spec.local.clone(),
                remotes: spec.remotes.clone(),
            }
        } else {
            let (local, remotes) = self.settle_backlog.front().cloned().unwrap_or_else(|| {
                (
                    unmatched_locals[0].clone(),
                    self.last_confirmed_remotes
                        .iter()
                        .zip(unmatched_remotes.iter())
                        .map(|(last, arrived)| arrived.first().cloned().unwrap_or_else(|| self.world.predict(last)))
                        .collect(),
                )
            });
            Frame {
                tick: self.settled_tick,
                state: &self.settled_state,
                local,
                remotes,
            }
        };
        Ok(Advance { frame, confirmed })
    }

    /// Settle what the queued input confirms, without new local input: how a
    /// session ends.
    ///
    /// Once the local player's input has ended (a replay's last tick, or the
    /// tick both peers agreed a match ends on), call this instead of
    /// [`advance`](Session::advance), once a tick, while the remotes' last
    /// inputs arrive. Like `advance` it matches the queued inputs and folds the
    /// confirmed rows into the settled state, promoting the speculations whose
    /// predictions held and simulating the rest, but it settles every confirmed
    /// row, however far past the present target, and speculates nothing: no
    /// tick past the last local input is simulated. It returns the rows it
    /// settled; there is no frame, so present [`settled_state`](Session::settled_state),
    /// which catches up with the frames presented before as the rows settle.
    /// Every local input has settled once [`is_drained`](Session::is_drained).
    ///
    /// The session goes on from where it is if `advance` is called again.
    ///
    /// # Errors
    ///
    /// As for [`advance`](Session::advance), and the session stays consistent
    /// the same way.
    pub fn drain(&mut self) -> Result<Vec<Confirmed<'_, W>>, W::Error> {
        self.begin_call();
        self.match_inputs();
        self.settle_to(self.confirm_frontier)?;
        let rows = std::mem::take(&mut self.settled_rows);
        Ok(lend(rows, &self.trail, &self.settled_state))
    }

    /// Start a call: forget the last one's rollback depth, and recycle the
    /// states the rows it returned lent (unless a call that failed left rows
    /// to return, whose states wait with them).
    fn begin_call(&mut self) {
        self.last_misprediction_depth = 0;
        if self.settled_rows.is_empty() {
            for state in self.trail.drain(..) {
                self.world.recycle(state);
            }
        }
    }

    /// Match the queued inputs into confirmed rows, onto the settle backlog.
    /// Returns the inputs left unmatched: the local ones, and each remote's.
    fn match_inputs(&mut self) -> (Vec<W::Input>, Vec<Vec<W::Input>>) {
        let (committable, unmatched_locals, unmatched_remotes) = self.input_queue.drain_matched();
        self.confirm_frontier += committable.len() as u32;
        self.settle_backlog.extend(committable);
        (unmatched_locals, unmatched_remotes)
    }

    /// Number of local inputs buffered but not yet confirmed against every
    /// remote's input.
    pub fn local_queue_length(&self) -> usize {
        self.input_queue.local_queue_length()
    }

    /// Number of inputs received from remote peer `remote` not yet matched
    /// into a confirmed tick.
    pub fn remote_queue_length(&self, remote: usize) -> usize {
        self.input_queue.remote_queue_length(remote)
    }

    /// How many rows the next [`advance`](Session::advance) could confirm from
    /// buffered remote input alone — the furthest-behind remote's backlog (see
    /// [`Queue::matchable`](crate::Queue::matchable)). Nonzero means advancing
    /// will drain the local queue rather than only grow it: the signal a stall
    /// guard needs so a full queue that is still being fed by the peer keeps
    /// settling instead of deadlocking.
    pub fn matchable(&self) -> usize {
        self.input_queue.matchable()
    }

    /// How far local input leads the furthest-behind remote's input, in ticks
    /// (clamped to [`i16`]).
    ///
    /// This is the input queue's signed worst-case [`lead`](crate::Queue::lead),
    /// surfaced for clock sync. It is each peer's half of the clock-sync
    /// handshake: you send it to every remote with every input, and each
    /// remote's own value comes back via
    /// [`add_remote_input`](Session::add_remote_input). The difference of the
    /// two is that peer's contribution to the [`skew`](Session::skew) used to
    /// keep the simulations aligned.
    pub fn local_tick_advantage(&self) -> i16 {
        self.input_queue.lead().clamp(i16::MIN as i32, i16::MAX as i32) as i16
    }

    /// The tick advantage remote peer `remote` last reported (via the
    /// `tick_advantage` argument to [`add_remote_input`](Session::add_remote_input)).
    pub fn last_remote_tick_advantage(&self, remote: usize) -> i16 {
        self.last_remote_tick_advantages[remote]
    }

    /// The clock-sync hint for the next tick to advance: the worst case, over
    /// the remotes, of this side's lead over that peer minus the advantage the
    /// peer last reported.
    ///
    /// Positive means this client is running ahead of at least one remote and
    /// should slow down (e.g. occasionally stall a frame) so the simulations
    /// converge and the prediction window stays small; zero or negative means
    /// nobody is waiting on us. With everyone throttling on their own skew,
    /// the group converges toward its slowest member.
    ///
    /// Read this *before* [`advance`](Session::advance): it reflects the local
    /// advantage at the point the peers read the value you ship them, which is
    /// before this tick's local input is enqueued. Reading it afterward would
    /// fold that just-enqueued input into the local half and bias the skew up by
    /// one.
    pub fn skew(&self) -> i32 {
        (0..self.last_remote_tick_advantages.len())
            .map(|i| self.input_queue.lead_over(i) - self.last_remote_tick_advantages[i] as i32)
            .max()
            .expect("session has at least one remote")
    }

    /// The signed balance of the latest presented frame around the speculation
    /// boundary — worst-case `lead - present_delay`, spanning both the
    /// speculative-depth and headroom sides so a single value covers both.
    /// (Floor the positive side for the plain speculative depth; negate and
    /// floor the other for the headroom.)
    ///
    /// This is *not* the raw local-over-remote lead. The presented frame is
    /// `frontier - present_delay`, so the present delay absorbs the first
    /// `present_delay` ticks of lead before any speculation is needed; only the
    /// excess is actually rendered into the speculative tail. So:
    ///
    /// * positive — the presented frame speculates that many ticks past the last
    ///   fully confirmed input;
    /// * zero — the frame is confirmed and sitting exactly at the boundary;
    /// * negative — the frame is confirmed with `-balance` ticks of *headroom*
    ///   (speculation-free buffer) still to spend before speculation begins.
    ///
    /// Clock-sync leniency keys off the sign: a positive
    /// [`skew`](Session::skew) only starts costing presentation quality once the
    /// balance reaches 0, so throttling callers gate engagement on it.
    pub fn speculation_balance(&self) -> i32 {
        self.input_queue.lead().max(0) - self.present_delay as i32
    }

    /// How many speculative frames the most recent [`advance`](Session::advance)
    /// discarded and re-simulated because a confirmed remote input contradicted
    /// the prediction — the instantaneous rollback depth for that frame. 0 when
    /// the frame promoted its predictions cleanly (or didn't settle). A telemetry
    /// signal: spikes mark mispredictions, unlike the steady-state
    /// [`speculation_balance`](Session::speculation_balance).
    pub fn last_misprediction_depth(&self) -> u32 {
        self.last_misprediction_depth
    }

    /// Record an input received from remote peer `remote`.
    ///
    /// * `remote` — which peer sent it (the slot index, `0..num_remotes`).
    /// * `input` — that player's input for their next unmatched tick.
    /// * `tick_advantage` — the peer's reported
    ///   [`local_tick_advantage`](Session::local_tick_advantage), used to
    ///   compute clock [`skew`](Session::skew).
    ///
    /// Call this whenever remote inputs arrive; they are matched into
    /// confirmed ticks on the next [`advance`](Session::advance).
    pub fn add_remote_input(&mut self, remote: usize, input: W::Input, tick_advantage: i16) {
        self.input_queue.add_remote_input(remote, input);
        self.last_remote_tick_advantages[remote] = tick_advantage;
    }

    /// Fold confirmed rows into the settled state up to `target`, promoting the
    /// speculative snapshots whose remote inputs all matched and rolling back to
    /// re-simulate where they didn't. Each row is told to the world as it
    /// settles and kept in `settled_rows` to be returned. No-op if already
    /// settled to or past `target`.
    ///
    /// On an error, the rows settled before it stay settled and the rest stay in
    /// the backlog, for the next call to retry.
    fn settle_to(&mut self, target: u32) -> Result<(), W::Error> {
        let to_settle = target.saturating_sub(self.settled_tick) as usize;
        if to_settle == 0 {
            return Ok(());
        }
        debug_assert!(self.settle_backlog.len() >= to_settle);

        // Longest prefix of the confirmed rows whose remotes all match what we
        // used speculatively — these can be promoted with no re-sim. (Slots
        // that speculated with an already-arrived real input match trivially.)
        let mut promote = 0;
        while promote < to_settle
            && promote < self.speculations.len()
            && self.speculations[promote].remotes == self.settle_backlog[promote].1
        {
            promote += 1;
        }

        // Promote the correctly-predicted prefix: slide the settled cap up over
        // the speculative snapshots, which are byte-exact because their remote
        // inputs equalled the real ones. The live simulation is not touched.
        for _ in 0..promote {
            let spec = self.speculations.pop_front().unwrap();
            let (local, remotes) = self.settle_backlog.pop_front().unwrap();
            assert_eq!(spec.tick, self.settled_tick + 1);
            self.replace_settled(spec.state);
            self.settle_row(local, remotes, Settlement::Promoted, true);
        }
        if promote > 0 {
            self.note_last_confirmed_remotes();
        }

        // Anything past the matched prefix descends from a wrong prediction (or
        // was never speculated): discard the speculative tail and re-simulate the
        // remaining confirmed rows authoritatively from the settled state. We
        // re-step the whole corrected tail and `save` only its final tick, and
        // settle the rows only once that has succeeded.
        if promote < to_settle {
            // The speculative tail we're throwing away — the rollback depth for
            // this frame (0 when there was simply nothing speculated yet).
            self.last_misprediction_depth = self.speculations.len() as u32;
            for spec in self.speculations.drain(..) {
                self.world.recycle(spec.state);
            }
            let simulate = to_settle - promote;
            // Rewind, unless the world is parked at the settled state already:
            // nothing was speculated past it, or every speculation was promoted
            // and the rows left were never speculated.
            if self.parked != Some(self.settled_tick) {
                self.parked = None;
                self.world.load(&self.settled_state)?;
            }
            self.parked = None;
            for (local, remotes) in self.settle_backlog.iter().take(simulate) {
                self.world.step(local, remotes)?;
            }
            let resettled = self.world.save()?;
            self.parked = Some(self.settled_tick + simulate as u32);
            // The new settled state is the last row's; the rows before it
            // have none, so it replaces the old one before any of them settles.
            self.replace_settled(resettled);
            for i in 0..simulate {
                let (local, remotes) = self.settle_backlog.pop_front().unwrap();
                self.settle_row(local, remotes, Settlement::Simulated, i + 1 == simulate);
            }
            self.note_last_confirmed_remotes();
        }

        debug_assert_eq!(self.settled_tick, target);
        Ok(())
    }

    /// Make `state` the settled state, before the rows it comes after settle.
    /// The one it replaces is the newest settled row's: kept in the trail if
    /// that row is still to be returned, and recycled otherwise (a returned
    /// row's, or the initial state).
    fn replace_settled(&mut self, state: W::State) {
        let displaced = std::mem::replace(&mut self.settled_state, state);
        if self.settled_rows.is_empty() {
            self.world.recycle(displaced);
        } else {
            self.trail.push(displaced);
        }
    }

    /// The next row joins the settled state (which is already the state after
    /// it, if `has_state`): tell the world, and keep the row to return.
    fn settle_row(&mut self, local: W::Input, remotes: Box<[W::Input]>, settlement: Settlement, has_state: bool) {
        let tick = self.settled_tick;
        self.settled_tick += 1;
        let row = Confirmed {
            tick,
            local,
            remotes,
            settlement,
            state: has_state.then_some(&self.settled_state),
        };
        self.world.settled(&row);
        let Confirmed { local, remotes, .. } = row;
        self.settled_rows.push(SettledRow {
            tick,
            local,
            remotes,
            settlement,
            has_state,
        });
    }

    /// Prediction only needs the newest settled remote row, so clone that
    /// boundary once per batch of settled rows rather than every row.
    fn note_last_confirmed_remotes(&mut self) {
        let last = self.settled_rows.last().expect("a row has settled");
        self.last_confirmed_remotes = last.remotes.clone();
    }

    /// Extend the speculative buffer up to `target` using real local inputs and,
    /// per remote slot, the real input where one has already arrived (a tick can
    /// be unconfirmed because a *different* remote is still missing) or a
    /// predicted one where it hasn't — simulating only the ticks not already
    /// covered. The live simulation is normally parked at the speculation
    /// frontier, so each new tick is a plain forward [`step`](World::step)
    /// followed by a [`save`](World::save).
    fn speculate_to(
        &mut self,
        target: u32,
        unmatched_locals: &[W::Input],
        unmatched_remotes: &[Vec<W::Input>],
    ) -> Result<(), W::Error> {
        assert_eq!(
            self.settled_tick, self.confirm_frontier,
            "speculation only runs once the settled cap has caught up to the confirmed frontier"
        );
        let frontier = self.settled_tick + self.speculations.len() as u32;
        if frontier >= target {
            return Ok(());
        }
        // Continue from the frontier's snapshot (the world is parked there
        // unless `world_mut` or an error moved it).
        if self.parked != Some(frontier) {
            self.parked = None;
            let at = self.speculations.back().map_or(&self.settled_state, |spec| &spec.state);
            self.world.load(at)?;
            self.parked = Some(frontier);
        }
        // Each remote's prediction chain continues from the newest speculated
        // value for that slot (real or predicted), or from the last confirmed
        // input when nothing is speculated yet.
        let mut chain: Box<[W::Input]> = match self.speculations.back() {
            Some(spec) => spec.remotes.clone(),
            None => self.last_confirmed_remotes.clone(),
        };
        while (self.settled_tick + self.speculations.len() as u32) < target {
            // `unmatched_locals[k]` (and `unmatched_remotes[i][k]`) hold the
            // input for tick `confirm_frontier + k`; here `confirm_frontier ==
            // settled_tick`, so the inputs for the next speculative tick are
            // at `speculations.len()`.
            let k = self.speculations.len();
            let local = unmatched_locals[k].clone();
            let tick = self.settled_tick + k as u32 + 1;
            let remotes: Box<[W::Input]> = chain
                .iter()
                .enumerate()
                .map(|(i, last)| match unmatched_remotes[i].get(k) {
                    Some(real) => real.clone(),
                    None => self.world.predict(last),
                })
                .collect();
            chain = remotes.clone();
            self.parked = None;
            self.world.step(&local, &remotes)?;
            let state = self.world.save()?;
            self.parked = Some(tick);
            self.speculations.push_back(Speculation {
                tick,
                state,
                local,
                remotes,
            });
        }
        Ok(())
    }
}

/// The rows to return, each with the state after it where the session kept
/// one: the newest settled row's is the settled state, the others' are in the
/// trail, in order.
fn lend<'a, W: World>(
    rows: Vec<SettledRow<W::Input>>,
    trail: &'a [W::State],
    settled: &'a W::State,
) -> Vec<Confirmed<'a, W>> {
    let newest = rows.len().wrapping_sub(1);
    let mut trail = trail.iter();
    let lent = rows
        .into_iter()
        .enumerate()
        .map(|(i, row)| Confirmed {
            tick: row.tick,
            local: row.local,
            remotes: row.remotes,
            settlement: row.settlement,
            state: if i == newest {
                Some(settled)
            } else if row.has_state {
                Some(trail.next().expect("a state for every row that has one"))
            } else {
                None
            },
        })
        .collect();
    debug_assert!(trail.next().is_none(), "the trail holds the returned rows' states only");
    lent
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[derive(Default)]
    struct Counters {
        restores: usize,
        steps: usize,
        recycles: usize,
        /// The furthest tick the world simulated.
        deepest: usize,
        /// The rows the world was told settled: (tick, settlement, the tick of
        /// the state that came with it).
        settled: Vec<(u32, Settlement, Option<usize>)>,
        /// World calls (step, save, load) so far, and the ones that fail.
        calls: usize,
        failing: Vec<usize>,
    }

    /// A world call failed (see `Counters::failing`).
    #[derive(Debug, PartialEq)]
    struct Failed;

    /// A deterministic world whose state is the full ordered history of applied
    /// `(local, remotes)` rows — so the settled state can be checked byte-for-byte
    /// against a ground-truth fold of the confirmed inputs.
    ///
    /// `load` refuses the state the world is parked at (the session never
    /// asks for it, unless a failure left the world somewhere it can't know),
    /// so every test checks that too, and the rollback counter counts genuine
    /// rewinds. `settled` checks that the world's last simulation of each
    /// settled row was on the row's inputs.
    struct W {
        parked: Vec<(u8, Vec<u8>)>,
        counters: Arc<Mutex<Counters>>,
        /// A call failed and left the world somewhere the session can't know.
        lost: bool,
    }

    impl W {
        /// Count a world call, and fail it if the test says so, leaving the
        /// world somewhere else (a bogus row further on).
        fn call(&mut self) -> Result<(), Failed> {
            let mut c = self.counters.lock().unwrap();
            c.calls += 1;
            if c.failing.contains(&c.calls) {
                self.parked.push((0xEE, vec![0xEE]));
                self.lost = true;
                return Err(Failed);
            }
            Ok(())
        }
    }

    impl World for W {
        type Input = u8;
        type State = Vec<(u8, Vec<u8>)>;
        type Error = Failed;

        fn step(&mut self, local: &u8, remotes: &[u8]) -> Result<(), Failed> {
            self.call()?;
            self.parked.push((*local, remotes.to_vec()));
            let mut c = self.counters.lock().unwrap();
            c.steps += 1;
            c.deepest = c.deepest.max(self.parked.len());
            Ok(())
        }
        fn save(&mut self) -> Result<Vec<(u8, Vec<u8>)>, Failed> {
            self.call()?;
            Ok(self.parked.clone())
        }
        fn load(&mut self, state: &Vec<(u8, Vec<u8>)>) -> Result<(), Failed> {
            if !self.lost {
                assert_ne!(
                    self.parked.len(),
                    state.len(),
                    "the session loaded the tick the world is parked at"
                );
            }
            self.call()?;
            self.counters.lock().unwrap().restores += 1;
            self.parked = state.clone();
            self.lost = false;
            Ok(())
        }
        // Repeat-predict: assume the remote keeps doing what they were doing.
        fn predict(&self, last: &u8) -> u8 {
            *last
        }
        fn recycle(&mut self, _state: Vec<(u8, Vec<u8>)>) {
            self.counters.lock().unwrap().recycles += 1;
        }
        fn settled(&mut self, row: &Confirmed<'_, Self>) {
            assert_eq!(
                self.parked.get(row.tick as usize),
                Some(&(row.local, row.remotes.to_vec())),
                "the world's last simulation of tick {} was on its real inputs",
                row.tick
            );
            let state = row.state.map(|s| s.len());
            self.counters
                .lock()
                .unwrap()
                .settled
                .push((row.tick, row.settlement, state));
        }
    }

    fn truth(locals: &[u8], remotes: &[&[u8]]) -> Vec<(u8, Vec<u8>)> {
        locals
            .iter()
            .enumerate()
            .map(|(t, &l)| (l, remotes.iter().map(|r| r[t]).collect()))
            .collect()
    }

    fn session(present_delay: u32, num_remotes: usize, counters: Arc<Mutex<Counters>>) -> Session<W> {
        Session::new(SessionParams {
            present_delay,
            initial_remotes: vec![0; num_remotes],
            initial_state: vec![],
            world: W {
                parked: vec![],
                counters,
                lost: false,
            },
        })
    }

    fn advance<'a>(session: &'a mut Session<W>, local: u8, confirmed: &mut Vec<(u8, Vec<u8>)>) -> Frame<'a, W> {
        let Advance { frame, confirmed: rows } = session.advance(local).unwrap();
        take_rows(rows, confirmed);
        frame
    }

    /// Add the rows to `confirmed`, checking each: ticks follow on with no
    /// gap, the newest row comes with the settled state, and every state a
    /// row comes with is the one after it (the history of the rows so far).
    fn take_rows(rows: Vec<Confirmed<'_, W>>, confirmed: &mut Vec<(u8, Vec<u8>)>) {
        let newest = rows.len().wrapping_sub(1);
        for (i, row) in rows.into_iter().enumerate() {
            assert_eq!(row.tick as usize, confirmed.len(), "rows come in tick order");
            confirmed.push((row.local, row.remotes.into_vec()));
            assert!(
                i != newest || row.state.is_some(),
                "the newest row has the settled state"
            );
            if row.settlement == Settlement::Promoted {
                assert!(row.state.is_some(), "a promoted row has its snapshot");
            }
            if let Some(state) = row.state {
                assert_eq!(state, confirmed, "the state after row {}", row.tick);
            }
        }
    }

    /// With remote input arriving late and every prediction wrong (distinct
    /// remotes, repeat-predictor), the settled state must stay a correct prefix of
    /// the ground truth at every frame and end up exactly equal — i.e. rollback
    /// re-simulation always corrects the mispredicted tail.
    #[test]
    fn settles_correctly_through_mispredictions() {
        let counters = Arc::new(Mutex::new(Counters::default()));
        let mut confirmed = Vec::new();
        let locals = [10u8, 11, 12, 13, 14, 15, 16, 17];
        let remotes = [20u8, 21, 22, 23, 24, 25, 26, 27];
        let truth = truth(&locals, &[&remotes]);
        let n = locals.len();
        let remote_delay = 2;

        let mut s = session(0, 1, counters.clone());

        // n real frames plus a couple to flush the present target to the end.
        for i in 0..n + remote_delay {
            if i >= remote_delay && i - remote_delay < n {
                s.add_remote_input(0, remotes[i - remote_delay], 0);
            }
            let local = if i < n { locals[i] } else { 99 };
            advance(&mut s, local, &mut confirmed);
            // The authoritative settled state is always a correct prefix of truth.
            let st = s.settled_state();
            assert_eq!(st.as_slice(), &truth[..st.len()], "settled diverged at frame {i}");
        }

        assert_eq!(
            s.settled_state().as_slice(),
            truth.as_slice(),
            "did not settle the full round"
        );
        assert_eq!(
            confirmed.as_slice(),
            truth.as_slice(),
            "returned the wrong confirmed rows"
        );
        // Mispredictions actually happened, so rollback re-sim ran.
        assert!(counters.lock().unwrap().restores > 0, "expected rollbacks");
        // Every discarded snapshot (cleared speculations, displaced settled
        // states) must be offered back to the world for reuse.
        assert!(counters.lock().unwrap().recycles > 0, "expected recycled states");
    }

    /// The stall-guard invariant the live drive loop depends on. A local queue
    /// that has run far ahead of a silent peer reports `matchable() == 0` —
    /// nothing is confirmable, so parking there is correct. But the moment the
    /// peer's inputs arrive (e.g. the resend burst after a transparent
    /// reconnect) `matchable()` turns positive and a *single* `advance` drains
    /// the whole backlog. `advance` is the only thing that drains the local
    /// queue, so a guard that parked on a full queue regardless of `matchable`
    /// (as the drive loop once did) would deadlock here: the buffered remote
    /// inputs would pile up forever unconsumed and the stall would never clear
    /// — the link "reconnects but never resumes".
    #[test]
    fn matchable_gates_stall_recovery() {
        let counters = Arc::new(Mutex::new(Counters::default()));
        let mut s = session(0, 1, counters);

        // The peer goes silent: advance a long run with no remote input. Each
        // advance enqueues a local input with nothing to match, so the local
        // queue climbs and nothing becomes confirmable.
        let backlog = 200u32;
        for i in 0..backlog {
            assert!(s.advance(i as u8).unwrap().confirmed.is_empty());
        }
        assert_eq!(
            s.local_queue_length(),
            backlog as usize,
            "every unmatched local input should still be queued"
        );
        assert_eq!(
            s.matchable(),
            0,
            "a silent peer offers nothing to confirm — the guard parks"
        );

        // The peer's inputs arrive all at once (the reconnect resend burst),
        // buffered by add_remote_input alone — which never drains the queue.
        for i in 0..backlog {
            s.add_remote_input(0, i as u8, 0);
        }
        assert_eq!(
            s.local_queue_length(),
            backlog as usize,
            "buffering remote input must not drain"
        );
        assert_eq!(
            s.matchable(),
            backlog as usize,
            "buffered remote inputs are now matchable — the guard must let advance run"
        );

        // A single advance settles the whole backlog: the queue drops back
        // below the stall cap instead of deadlocking.
        let advanced = s.advance(backlog as u8).unwrap();
        assert_eq!(advanced.confirmed.len(), backlog as usize);
        assert!(advanced
            .confirmed
            .iter()
            .enumerate()
            .all(|(tick, row)| row.tick == tick as u32
                && row.local == tick as u8
                && row.remotes.as_ref() == [tick as u8]));
        assert!(
            s.local_queue_length() <= 1,
            "advance must drain the matched backlog, got {}",
            s.local_queue_length()
        );
    }

    /// When predictions hold (remote held constant so repeat-predict is right),
    /// confirming a tick must promote the existing speculation with no extra
    /// simulation: the total step count equals the number of distinct ticks ever
    /// simulated, never re-running a tick that was already speculated correctly.
    #[test]
    fn correct_predictions_promote_without_resim() {
        let counters = Arc::new(Mutex::new(Counters::default()));
        let mut confirmed = Vec::new();
        // Constant remote: predict(last) == next, so every speculation is right.
        let locals = [1u8, 2, 3, 4, 5, 6, 7, 8];
        let remotes = [9u8; 8];
        let truth = truth(&locals, &[&remotes]);
        let n = locals.len();
        let remote_delay = 2;

        let mut s = session(0, 1, counters.clone());
        for i in 0..n + remote_delay {
            if i >= remote_delay && i - remote_delay < n {
                s.add_remote_input(0, remotes[i - remote_delay], 0);
            }
            let local = if i < n { locals[i] } else { 99 };
            advance(&mut s, local, &mut confirmed);
        }

        assert_eq!(s.settled_state().as_slice(), truth.as_slice());
        assert_eq!(confirmed.as_slice(), truth.as_slice());
        // Each of the n ticks is simulated exactly once (as a speculation that is
        // then promoted) — no tick is re-simulated, so steps == n. Extra dummy
        // ticks may add a few more speculative steps, but never re-run a settled
        // tick; the key invariant is no redundant re-sim of confirmed ticks.
        let steps = counters.lock().unwrap().steps;
        assert!(steps >= n, "every confirmed tick must be simulated at least once");
        assert_eq!(
            counters.lock().unwrap().restores,
            0,
            "correct predictions must not roll back"
        );
        // Each promotion displaces exactly one settled state, and with no
        // rollbacks that's the only discard path: one recycle per settled tick.
        assert_eq!(
            counters.lock().unwrap().recycles,
            s.settled_state().len(),
            "every promotion must recycle the displaced settled state"
        );
    }

    /// Raising `present_delay` mid-session shrinks `target` below the speculation
    /// frontier already built, so the deepest buffered spec sits *past* the new
    /// target. `advance` must present the spec at `target` (not the deepest) and
    /// keep settling correctly — regression for a panic when the two diverged.
    #[test]
    fn present_delay_increase_presents_target_not_tail() {
        let counters = Arc::new(Mutex::new(Counters::default()));
        let mut confirmed = Vec::new();
        // Constant remote: every prediction holds, so the deep speculation tail is
        // never cleared by a rollback — exactly the case that leaves the buffer
        // longer than `target` after the delay bump.
        let locals: Vec<u8> = (1..=24).collect();
        let remotes = [9u8; 24];
        let truth = truth(&locals, &[&remotes]);
        let n = locals.len();
        // Remote lags far enough that even after the bump the settled cap stays
        // behind `target` (so `advance` takes the speculating branch, not the
        // settled one).
        let remote_delay = 6;

        // present_delay 0 first: speculation runs `remote_delay` ticks ahead of the
        // settled cap, building a deep buffer.
        let mut s = session(0, 1, counters.clone());
        for i in 0..12 {
            if i >= remote_delay {
                s.add_remote_input(0, remotes[i - remote_delay], 0);
            }
            advance(&mut s, locals[i], &mut confirmed);
        }

        // Bump present_delay: `target` drops below the buffer frontier built above,
        // so the deepest spec now sits past `target`. This used to panic on
        // `assert_eq!(spec.tick, target)`.
        s.set_present_delay(3);
        for i in 12..n + remote_delay {
            if i >= remote_delay && i - remote_delay < n {
                s.add_remote_input(0, remotes[i - remote_delay], 0);
            }
            let local = if i < n { locals[i] } else { 99 };
            let frame_tick = advance(&mut s, local, &mut confirmed).tick;
            // Settled stays a correct prefix of ground truth, and the presented
            // tick never runs behind the settled cap.
            let st = s.settled_state();
            assert_eq!(st.as_slice(), &truth[..st.len()], "settled diverged at frame {i}");
            assert!(frame_tick >= st.len() as u32, "presented before settled at frame {i}");
        }

        assert_eq!(s.settled_state().as_slice(), truth.as_slice());
        assert_eq!(confirmed.as_slice(), truth.as_slice());
    }

    /// Three players, each remote arriving with a different lag, every
    /// prediction wrong (distinct values, repeat-predictor): the settled state
    /// must stay a correct prefix of ground truth at every frame and end exactly
    /// equal, and the confirmed rows must be returned in order.
    #[test]
    fn three_player_settles_correctly_with_skewed_arrivals() {
        let counters = Arc::new(Mutex::new(Counters::default()));
        let mut confirmed = Vec::new();
        let locals = [10u8, 11, 12, 13, 14, 15, 16, 17];
        let remote_a = [20u8, 21, 22, 23, 24, 25, 26, 27];
        let remote_b = [30u8, 31, 32, 33, 34, 35, 36, 37];
        let truth = truth(&locals, &[&remote_a, &remote_b]);
        let n = locals.len();
        let delays = [1usize, 4usize];

        let mut s = session(0, 2, counters.clone());
        let max_delay = 4;
        for i in 0..n + max_delay {
            for (slot, (inputs, delay)) in [(&remote_a, delays[0]), (&remote_b, delays[1])].iter().enumerate() {
                if i >= *delay && i - *delay < n {
                    s.add_remote_input(slot, inputs[i - *delay], 0);
                }
            }
            let local = if i < n { locals[i] } else { 99 };
            advance(&mut s, local, &mut confirmed);
            let st = s.settled_state();
            assert_eq!(st.as_slice(), &truth[..st.len()], "settled diverged at frame {i}");
        }

        assert_eq!(s.settled_state().as_slice(), truth.as_slice());
        assert_eq!(confirmed.as_slice(), truth.as_slice());
        assert!(counters.lock().unwrap().restores > 0, "expected rollbacks");
    }

    /// A remote input that has already *arrived* — its tick merely unconfirmed
    /// because a different remote is still missing — must be used as-is by
    /// speculation, not second-guessed by the predictor. Remote A varies every
    /// tick (repeat-predict would be wrong every time) but arrives with lag 1,
    /// inside the present delay, so every speculated tick already has A's real
    /// input; remote B is constant (predictions hold) but arrives with lag 4.
    /// If speculation uses A's real inputs, nothing ever rolls back.
    #[test]
    fn arrived_inputs_beat_predictions_in_speculation() {
        let counters = Arc::new(Mutex::new(Counters::default()));
        let mut confirmed = Vec::new();
        let locals = [1u8, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12];
        let remote_a = [40u8, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51];
        let remote_b = [7u8; 12];
        let truth = truth(&locals, &[&remote_a, &remote_b]);
        let n = locals.len();

        let mut s = session(2, 2, counters.clone());
        let max_delay = 4;
        for i in 0..n + max_delay {
            if i >= 1 && i - 1 < n {
                s.add_remote_input(0, remote_a[i - 1], 0);
            }
            if i >= 4 && i - 4 < n {
                s.add_remote_input(1, remote_b[i - 4], 0);
            }
            let local = if i < n { locals[i] } else { 99 };
            advance(&mut s, local, &mut confirmed);
            let st = s.settled_state();
            assert_eq!(st.as_slice(), &truth[..st.len()], "settled diverged at frame {i}");
        }

        assert_eq!(s.settled_state().as_slice(), truth.as_slice());
        assert_eq!(confirmed.as_slice(), truth.as_slice());
        assert_eq!(
            counters.lock().unwrap().restores,
            0,
            "speculation second-guessed an arrived input"
        );
    }

    /// The session loads only to move the world. In the delay regime (the
    /// present delay covers the remote lag, so nothing is speculated) every
    /// row is simulated as it settles, from where the world is parked: no
    /// load at all. With speculation, a load comes only with a
    /// misprediction. (`W::load` also refuses the state the world is parked
    /// at, so every test checks that.)
    #[test]
    fn loads_only_to_move_the_world() {
        let counters = Arc::new(Mutex::new(Counters::default()));
        let mut confirmed = Vec::new();
        let locals: Vec<u8> = (0..16).collect();
        let remotes: Vec<u8> = (100..116).collect();
        let truth = truth(&locals, &[&remotes]);
        let remote_delay = 2;
        let mut s = session(3, 1, counters.clone());
        for i in 0..locals.len() + 4 {
            if i >= remote_delay && i - remote_delay < remotes.len() {
                s.add_remote_input(0, remotes[i - remote_delay], 0);
            }
            advance(&mut s, *locals.get(i).unwrap_or(&99), &mut confirmed);
        }
        assert_eq!(&confirmed[..], &truth[..confirmed.len()]);
        assert!(confirmed.len() >= locals.len());
        let c = counters.lock().unwrap();
        assert_eq!(c.restores, 0, "nothing was speculated, so nothing was loaded");
        assert_eq!(c.steps, confirmed.len(), "each row simulated once");
    }

    /// The host reaches the world between advances. Reading it changes
    /// nothing; changing it, even moving its live simulation, makes the
    /// session load its own snapshot before it steps the world again, and
    /// the settled state stays right.
    #[test]
    fn the_host_reaches_the_world() {
        let counters = Arc::new(Mutex::new(Counters::default()));
        let mut confirmed = Vec::new();
        let locals = [1u8, 2, 3, 4, 5, 6, 7, 8, 9, 10];
        let remotes = [9u8; 10];
        let truth = truth(&locals, &[&remotes]);
        let remote_delay = 2;
        let mut s = session(0, 1, counters.clone());
        for i in 0..locals.len() + remote_delay {
            if i >= remote_delay && i - remote_delay < remotes.len() {
                s.add_remote_input(0, remotes[i - remote_delay], 0);
            }
            // Where the world is parked: the newest speculation.
            let parked = s.world().parked.len();
            assert!(parked >= s.settled_state().len());
            if i == 5 {
                // Behind the session's back: the world is somewhere else.
                s.world_mut().parked.push((0xEE, vec![0xEE]));
            }
            advance(&mut s, *locals.get(i).unwrap_or(&99), &mut confirmed);
        }
        assert_eq!(s.settled_state().as_slice(), truth.as_slice());
        assert_eq!(confirmed.as_slice(), truth.as_slice());
        // Predictions all held: the only load is the one after `world_mut`.
        assert_eq!(counters.lock().unwrap().restores, 1);
    }

    /// A session ends by handing its world back, with what it keeps.
    #[test]
    fn a_session_hands_its_world_back() {
        let counters = Arc::new(Mutex::new(Counters::default()));
        let mut s = session(0, 1, counters.clone());
        for i in 0..4 {
            s.add_remote_input(0, i, 0);
            s.advance(i).unwrap();
        }
        let settled = s.settled_state().clone();
        let world = s.into_world();
        assert!(Arc::ptr_eq(&world.counters, &counters));
        assert_eq!(&world.parked[..settled.len()], settled.as_slice());
    }

    /// A session is `Send` when its world is.
    #[test]
    fn a_session_is_send() {
        fn send<T: Send>() {}
        send::<Session<W>>();
        let s = session(0, 1, Arc::new(Mutex::new(Counters::default())));
        let s = std::thread::spawn(move || s).join().unwrap();
        assert_eq!(s.local_frontier(), 0);
    }

    /// Each confirmed row says its tick, whether its speculation was promoted
    /// or it was simulated as it settled, and the state after it where the
    /// session kept one; the world is told the same rows, once each, in
    /// order. The remote holds still (predictions hold, rows are promoted),
    /// changes (a misprediction: simulated again), and lags past the present
    /// target in a burst (rows never speculated: simulated for the first
    /// time, several in one call, only the last with a state).
    #[test]
    fn rows_say_their_tick_and_how_they_settled() {
        let counters = Arc::new(Mutex::new(Counters::default()));
        let mut confirmed = Vec::new();
        let locals: Vec<u8> = (0..24).collect();
        let remotes: Vec<u8> = [&[5u8; 10][..], &[6; 6], &[7; 8]].concat();
        let truth = truth(&locals, &[&remotes]);
        let mut s = session(0, 1, counters.clone());
        let mut settlements = Vec::new();
        for i in 0..locals.len() + 8 {
            // Two ticks late, except a stall from 12 to 17 that then
            // delivers everything held back.
            let arrived: Vec<usize> = match i {
                12..=16 => vec![],
                17 => (10..16).collect(),
                _ if i >= 2 => vec![i - 2],
                _ => vec![],
            };
            for t in arrived.into_iter().filter(|&t| t < remotes.len()) {
                s.add_remote_input(0, remotes[t], 0);
            }
            let Advance { confirmed: rows, .. } = s.advance(*locals.get(i).unwrap_or(&99)).unwrap();
            settlements.extend(rows.iter().map(|r| (r.tick, r.settlement, r.state.is_some())));
            take_rows(rows, &mut confirmed);
        }
        assert_eq!(confirmed.as_slice(), truth.as_slice());
        let promoted = |t: u32| settlements[t as usize].1 == Settlement::Promoted;
        // The remote held still: promoted (the first row, confirmed before
        // anything was speculated, was simulated).
        assert!(!promoted(0) && (1..10).all(promoted), "{settlements:?}");
        // It changed at 10, in the burst, and at 16: those rows were
        // simulated again, the burst's in one call with only its last row's
        // state kept.
        assert!(!promoted(10) && !promoted(16), "{settlements:?}");
        assert!(
            (10..15).all(|t| settlements[t] == (t as u32, Settlement::Simulated, false)),
            "{settlements:?}"
        );
        assert_eq!(settlements[15], (15, Settlement::Simulated, true));
        // The world was told the same rows.
        let told = counters.lock().unwrap().settled.clone();
        assert_eq!(told.len(), truth.len());
        for (&(tick, how, state), &(t, h, kept)) in told.iter().zip(&settlements) {
            assert_eq!((tick, how, state.is_some()), (t, h, kept));
            assert!(state.is_none_or(|len| len == tick as usize + 1));
        }
    }

    /// A world call that fails leaves the session consistent: the local
    /// input is taken, the rows settled before the failure come back from
    /// the next call that succeeds, the next call retries the rest (loading
    /// first, since the failure left the world somewhere else), and the
    /// session settles exactly what it would have without the failure, each
    /// row returned and told to the world once. Tried with each world call of
    /// a mispredicting session failing in turn, and with runs of failures.
    #[test]
    fn a_world_error_leaves_the_session_consistent() {
        let locals: Vec<u8> = (10..30).collect();
        // Changing every tick: every prediction is wrong.
        let remotes: Vec<u8> = (40..60).collect();
        let truth = truth(&locals, &[&remotes]);
        let run = |failing: Vec<usize>| -> (usize, usize) {
            let counters = Arc::new(Mutex::new(Counters {
                failing,
                ..Counters::default()
            }));
            let mut s = session(1, 1, counters.clone());
            let mut confirmed = Vec::new();
            let mut errors = 0;
            for i in 0..locals.len() + 16 {
                if i >= 3 && i - 3 < remotes.len() {
                    s.add_remote_input(0, remotes[i - 3], 0);
                }
                match s.advance(*locals.get(i).unwrap_or(&99)) {
                    Ok(advanced) => take_rows(advanced.confirmed, &mut confirmed),
                    Err(Failed) => errors += 1,
                }
                assert_eq!(s.local_frontier(), i as u32 + 1, "the input is taken");
            }
            assert_eq!(confirmed.as_slice(), truth.as_slice());
            assert_eq!(s.settled_state().as_slice(), truth.as_slice());
            let c = counters.lock().unwrap();
            let told: Vec<u32> = c.settled.iter().map(|x| x.0).collect();
            assert_eq!(told, (0..truth.len() as u32).collect::<Vec<_>>());
            (c.calls, errors)
        };
        let (calls, errors) = run(vec![]);
        assert_eq!(errors, 0);
        for k in 1..=calls {
            for failing in [vec![k], (k..k + 3).collect()] {
                let (_, errors) = run(failing.clone());
                assert!(errors > 0, "call {k} failed");
            }
        }
    }

    /// `drain` ends a session: once the local input has ended, it settles
    /// what the remote's last inputs confirm, however far past the present
    /// target, without speculating, so no tick past the last input is ever
    /// simulated; every row comes back once, and the session is drained when
    /// the last one has settled. (With `advance` alone, the last
    /// `present_delay` ticks never settle unless the host makes up input past
    /// the end.)
    #[test]
    fn drain_settles_the_last_ticks() {
        let counters = Arc::new(Mutex::new(Counters::default()));
        let mut confirmed = Vec::new();
        let locals: Vec<u8> = (0..20).collect();
        // Changing now and then: some predictions fail.
        let remotes: Vec<u8> = (0..20).map(|t| t / 3).collect();
        let truth = truth(&locals, &[&remotes]);
        let remote_delay = 4;
        let mut s = session(2, 1, counters.clone());
        for (i, &local) in locals.iter().enumerate() {
            if i >= remote_delay {
                s.add_remote_input(0, remotes[i - remote_delay], 0);
            }
            advance(&mut s, local, &mut confirmed);
        }
        assert!(!s.is_drained());
        let mut drains = 0;
        let mut next = locals.len() - remote_delay;
        while !s.is_drained() {
            if next < remotes.len() {
                s.add_remote_input(0, remotes[next], 0);
                next += 1;
            }
            let rows = s.drain().unwrap();
            take_rows(rows, &mut confirmed);
            assert_eq!(s.settled_tick() as usize, confirmed.len());
            drains += 1;
            assert!(drains <= remote_delay, "the remote's last inputs settle as they arrive");
        }
        assert_eq!(s.settled_tick(), s.local_frontier());
        assert_eq!(confirmed.as_slice(), truth.as_slice());
        assert_eq!(s.settled_state().as_slice(), truth.as_slice());
        let c = counters.lock().unwrap();
        assert_eq!(c.deepest, locals.len(), "nothing simulated past the last input");
        assert!(c.restores > 0, "some predictions failed");
        // Draining a drained session settles nothing.
        drop(c);
        assert!(s.drain().unwrap().is_empty());
    }
}
