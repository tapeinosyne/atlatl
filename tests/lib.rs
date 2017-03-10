extern crate atlatl;
extern crate rand;
extern crate quickcheck;

use quickcheck::{quickcheck};
use std::collections::BTreeMap;

use atlatl::*;
use atlatl::fst::*;


#[test]
fn fst_output_matches_source_u32_u16() {
    fn property(btree: BTreeMap<Vec<u8>, u16>) -> bool {
        let b = fst::Builder::from_iter(btree.iter().map(|(k, &v)| (k, v))).unwrap();
        let fst : FST<u32, u16> = FST::from_builder(&b).unwrap();

        btree.iter().all(|(k, &from_btree)| {
            let from_fst = fst.get(k).unwrap();
            from_fst == from_btree
        })
    }

    quickcheck(property as fn(BTreeMap<Vec<u8>, u16>) -> bool);
}

#[test]
fn fst_output_matches_source_u32_i16() {
    fn property(btree: BTreeMap<Vec<u8>, i16>) -> bool {
        let b = fst::Builder::from_iter(btree.iter().map(|(k, &v)| (k, v))).unwrap();
        let fst : FST<u32, i16> = FST::from_builder(&b).unwrap();

        btree.iter().all(|(k, &from_btree)| {
            let from_fst = fst.get(k).unwrap();
            from_fst == from_btree
        })
    }

    quickcheck(property as fn(BTreeMap<Vec<u8>, i16>) -> bool);
}


#[test]
fn fst_reap() {
    let pairs = &[("", 3), ("a", 0), ("ab", 1), ("abc", 2)];
    let b = fst::Builder::from_iter(pairs.iter().cloned()).unwrap();
    let fst : FST<u32, i16> = FST::from_builder(&b).unwrap();

    let reaper = fst.reap("abcd".as_bytes());
    assert!((1, Some(5)) == reaper.size_hint());
    let reaped : Vec<_> = reaper.collect();
    assert!(4 == reaped.len());
    assert!(vec![(0, 3), (1, 0), (2, 1), (3, 2)] == reaped);
}
