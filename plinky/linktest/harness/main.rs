mod prerequisites;
mod tests;
mod utils;

use std::collections::HashMap;
use crate::tests::Test;

macro_rules! linktest {
    ($name:ident, files[$($file:expr),*]) => {
        #[test]
        fn $name() {
            let mut files = HashMap::new();
            $(files.insert($file.rsplit_once('/').unwrap().1, include_bytes!($file) as &[u8]);)*
            Test {
                name: stringify!($name),
                files,
            }.run().unwrap();
        }
    }
}

include!(concat!(env!("OUT_DIR"), "/linktest_definition.rs"));
