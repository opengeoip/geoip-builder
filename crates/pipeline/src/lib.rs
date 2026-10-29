pub mod arin_check;
pub mod build;
pub mod candidates;
pub mod catalog;
pub mod coverage;
pub mod export;
pub mod fetch;
pub mod sources;
pub mod truth;

mod io;

pub type Log<'a> = dyn FnMut(String) + 'a;
