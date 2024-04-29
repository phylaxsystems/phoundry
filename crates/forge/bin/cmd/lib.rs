#[path = "mod.rs"]
mod cmd;
use cmd::{install, watch};

mod test;
pub use test::{FilterArgs, ProjectPathsAwareFilter};

#[macro_use]
extern crate tracing;
