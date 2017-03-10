#![feature(test)]
#![allow(non_upper_case_globals, unused_must_use)]

extern crate atlatl;
extern crate fnv;
extern crate fst;
extern crate rand;
extern crate test;
#[macro_use] extern crate lazy_static;

use fnv::FnvHashMap;
use rand::{thread_rng, Rand, Rng, sample};
use std::collections::{BTreeMap, HashMap};
use std::iter::FromIterator;
use test::{Bencher, black_box};

use atlatl::fst::*;


lazy_static! {
    static ref small : Vec<(Vec<u8>, u32)> = pairs(1000, (0, 16));
    static ref sample_s_s : Vec<&'static [u8]> = key_sample(small.iter(), 4, 16);
    static ref sample_s_m : Vec<&'static [u8]> = key_sample(small.iter(), 8, 16);
    static ref sample_s_l : Vec<&'static [u8]> = key_sample(small.iter(), 16, 16);

    static ref medium : Vec<(Vec<u8>, u32)> = pairs(10000, (0, 16));
    static ref sample_m_s : Vec<&'static [u8]> = key_sample(medium.iter(), 4, 16);
    static ref sample_m_m : Vec<&'static [u8]> = key_sample(medium.iter(), 8, 16);
    static ref sample_m_l : Vec<&'static [u8]> = key_sample(medium.iter(), 16, 16);

    static ref large : Vec<(Vec<u8>, u32)> = pairs(50000, (0, 16));
    static ref sample_l_s : Vec<&'static [u8]> = key_sample(large.iter(), 4, 16);
    static ref sample_l_m : Vec<&'static [u8]> = key_sample(large.iter(), 8, 16);
    static ref sample_l_l : Vec<&'static [u8]> = key_sample(large.iter(), 16, 16);
}


fn pairs<U>(n : usize, (l, r) : (usize, usize)) -> Vec<(Vec<u8>, U)>
    where U : Ord + Rand
{
    let mut rng = thread_rng();
    let mut v : Vec<(Vec<u8>, U)> =
        (0 .. n).map(|_| {
            let l_k = rng.gen_range(l, r);
            (rng.gen_iter::<u8>().take(l_k).collect(), rng.gen::<U>())
    }).collect();
    v.sort();
    v.dedup_by(|a, b| a.0 == b.0);
    v
}

fn key_sample<'a, I, T>(kvs : I, max_len : usize, amount : usize) -> Vec<&'a [u8]>
    where I : Iterator<Item = &'a (Vec<u8>, T)>
        , T : 'a
{
    let keys = kvs.map(|&(ref k, _)| k.as_slice())
                  .filter(|k| k.len() <= max_len);
    sample(&mut thread_rng(), keys, amount)
}


macro_rules! _bench_coll {
    ($name:ident, $collection:ident, $source:ident, $sample:ident) => {
        #[bench]
        fn $name(b: &mut Bencher) {
            let iter = $source.iter().map(|&(ref k, v)| (k.as_slice(), v));
            let map : $collection<_, _> = $collection::from_iter(iter);
            let key = $sample[0];

            b.iter(|| {
                black_box(map.get(key));
            });
        }
    }
}

macro_rules! bench_fst {
    ($name:ident, $source:ident, $sample:ident) => {
        #[bench]
        fn $name(b: &mut Bencher) {
            let iter = $source.iter().map(|&(ref k, v)| (k.as_slice(), v));
            let fst_b = atlatl::fst::Builder::from_iter(iter).unwrap();
            let fst : FST<usize, _> = FST::from_builder(&fst_b).unwrap();
            let key = $sample[0];

            b.iter(|| black_box(fst.get(key)));
        }
    }
}

macro_rules! bench_rawfst {
    ($name:ident, $source:ident, $sample:ident) => {
        #[bench]
        fn $name(b: &mut Bencher) {
            use fst::raw::{Fst, Output};

            let mut fst_b = fst::raw::Builder::memory();
            let iter = $source.iter().map(|&(ref k, v)| (k.as_slice(), Output::new(v as u64)));
            fst_b.extend_iter(iter);
            let fst = Fst::from_bytes(fst_b.into_inner().unwrap()).unwrap();
            let key = $sample[0];

            b.iter(|| black_box(fst.get(key)));
        }
    }
}

macro_rules! bench_coll {
    ( $collection:ident
    , $id_small_short:ident, $id_small_mid:ident, $id_small_long:ident
    , $id_medium_short:ident, $id_medium_mid:ident, $id_medium_long:ident
    , $id_large_short:ident, $id_large_mid:ident, $id_large_long:ident
    ) => {
        _bench_coll! { $id_small_short, $collection, small, sample_s_s }
        _bench_coll! { $id_small_mid, $collection, small, sample_s_m }
        _bench_coll! { $id_small_long, $collection, small, sample_s_l }

        _bench_coll! { $id_medium_short, $collection, medium, sample_m_s }
        _bench_coll! { $id_medium_mid, $collection, medium, sample_m_m }
        _bench_coll! { $id_medium_long, $collection, medium, sample_m_l }

        _bench_coll! { $id_large_short, $collection, large, sample_l_s }
        _bench_coll! { $id_large_mid, $collection, large, sample_l_m }
        _bench_coll! { $id_large_long, $collection, large, sample_l_l }
    }
}


bench_coll! { HashMap
            , get_small_short_hashmap, get_small_mid_hashmap, get_small_long_hashmap
            , get_medium_short_hashmap, get_medium_mid_hashmap, get_medium_long_hashmap
            , get_large_short_hashmap, get_large_mid_hashmap, get_large_long_hashmap }

bench_coll! { FnvHashMap
            , get_small_short_fnvhashmap, get_small_mid_fnvhashmap, get_small_long_fnvhashmap
            , get_medium_short_fnvhashmap, get_medium_mid_fnvhashmap, get_medium_long_fnvhashmap
            , get_large_short_fnvhashmap, get_large_mid_fnvhashmap, get_large_long_fnvhashmap }

bench_coll! { BTreeMap
            , get_small_short_btree, get_small_mid_btree, get_small_long_btree
            , get_medium_short_btree, get_medium_mid_btree, get_medium_long_btree
            , get_large_short_btree, get_large_mid_btree, get_large_long_btree }

bench_fst! { get_small_short_fst, small, sample_s_s }
bench_fst! { get_small_mid_fst, small, sample_s_m }
bench_fst! { get_small_long_fst, small, sample_s_l }
bench_fst! { get_medium_short_fst, medium, sample_m_s }
bench_fst! { get_medium_mid_fst, medium, sample_m_m }
bench_fst! { get_medium_long_fst, medium, sample_m_l }
bench_fst! { get_large_short_fst, large, sample_l_s }
bench_fst! { get_large_mid_fst, large, sample_l_m }
bench_fst! { get_large_long_fst, large, sample_l_l }

bench_rawfst! { get_small_short_rawfst, small, sample_s_s }
bench_rawfst! { get_small_mid_rawfst, small, sample_s_m }
bench_rawfst! { get_small_long_rawfst, small, sample_s_l }
bench_rawfst! { get_medium_short_rawfst, medium, sample_m_s }
bench_rawfst! { get_medium_mid_rawfst, medium, sample_m_m }
bench_rawfst! { get_medium_long_rawfst, medium, sample_m_l }
bench_rawfst! { get_large_short_rawfst, large, sample_l_s }
bench_rawfst! { get_large_mid_rawfst, large, sample_l_m }
bench_rawfst! { get_large_long_rawfst, large, sample_l_l }
