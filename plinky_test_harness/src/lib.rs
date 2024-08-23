#![feature(test)]

extern crate test;

mod gather;
mod steps;
pub mod template;
mod tests;
pub mod utils;

pub use crate::gather::DefineSteps;
use crate::gather::{gather, DefineStepsFn};
pub use crate::steps::Step;
use crate::utils::err_str;
use std::path::Path;
pub use tests::{Arch, TestContext};

pub fn main(path: &Path, define_steps: DefineStepsFn) {
    let args = std::env::args().collect::<Vec<_>>();
    let tests = err_str(gather(path, define_steps)).unwrap();
    test::test_main(&args, tests, None);
}
