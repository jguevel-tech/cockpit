pub mod parser;
pub mod fetcher;
pub mod diff;
pub mod runner;

pub use runner::{run_ping, run_diff, PingReport, DiffReport};
