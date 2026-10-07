//! ptop — vtop-parity terminal system monitor.
//! CONTRACTS.md is law; spec (Desktop ptop-SPEC.md) is the plan.

pub mod app;
pub mod canvas;
pub mod capture;
pub mod chart;
pub mod cli;
pub mod colors;
pub mod content;
pub mod jsnum;
pub mod screen;
pub mod sensors;
pub mod table;
pub mod tags;
pub mod theme;
pub mod upgrade;
pub mod wrap;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const NPM_PACKAGE: &str = "@patebryant/ptop";

mod poll;

mod signals;
