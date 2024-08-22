#![feature(test)]

extern crate test;

mod gather;
pub mod legacy;
mod steps;
pub mod template;
mod tests;
pub mod utils;

pub use crate::gather::DefineSteps;
pub use crate::steps::Step;
pub use tests::{TestContext, Arch};
use std::path::Path;
use crate::utils::err_str;
use crate::gather::{gather, DefineStepsFn};

pub fn main(path: &Path, define_steps: DefineStepsFn) {
    let args = std::env::args().collect::<Vec<_>>();
    let tests = err_str(gather(path, define_steps)).unwrap();
    test::test_main(&args, tests, None);
}
