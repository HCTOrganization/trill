//! Host-provided destination for completed statistics. Sessions own no cache paths.

pub trait StatsSink: trill_platform::WasmNotSend + trill_platform::WasmNotSync {
    fn record(&self, replay_key: &std::path::Path, stats: &trill_match::analysis::MatchStats) -> std::io::Result<()>;
}
