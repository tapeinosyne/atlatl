extern crate atlatl;
extern crate rand;
extern crate quickcheck;

use quickcheck::{quickcheck};
use std::collections::BTreeMap;

use atlatl::*;
use atlatl::fst::*;


#[test]
fn fst_output_matches_source() {
    fn property(btree: BTreeMap<Vec<u8>, u16>) -> bool {
        let b = fst::builder::Builder::from_iter(btree.iter().map(|(k, &v)| (k, v)));
        let fst : FST<usize, u16> = FST::from_builder(&b);

        btree.iter().all(|(k, &from_btree)| {
            let from_fst = fst.get(k).unwrap();
            from_fst == from_btree
        })
    }

    quickcheck(property as fn(BTreeMap<Vec<u8>, u16>) -> bool);
}
