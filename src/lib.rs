pub mod config;
pub mod install;
pub mod package;
pub mod remove;
pub mod index;
pub mod meta;
pub mod verbosity;
pub mod version;

pub use verbosity::{is_verbose, set_verbose};