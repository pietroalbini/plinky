#![feature(test)]

extern crate test;

mod gather;
mod prerequisites;
mod tests;
mod utils;

use crate::gather::gather_tests;
use crate::utils::err_str;
use std::path::Path;

fn main() {
    let args = std::env::args().collect::<Vec<_>>();
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("linktest").join("tests");
    test::test_main(&args, err_str(gather_tests(&path)).unwrap(), None)
}
