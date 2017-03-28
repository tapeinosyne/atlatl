//! Paging structures for fast insertion in a Dart.

use fnv::FnvHashSet;


#[derive(Clone, Debug)]
pub struct IndexSegments {
    as_state : FnvHashSet<usize>,
    as_trans : FnvHashSet<usize>,
    block_size : usize,
}

impl IndexSegments {
    /// Settle the transitions labelled with `symbols` in the segments,
    /// returning their base index.
    pub fn settle(&mut self, symbols : &[u8]) -> Option<usize> {
        self.usher(symbols).map(|base| {
            self.affix_state(base);
            for &s in symbols { self.affix_trans(base + (1 + s as usize)) }
            base
        })
    }

    pub fn settle_index(&mut self, symbols : &[u8], i : usize) -> Option<usize> {
        self.as_state.get(&i).cloned()
            .map(|base| {
                self.affix_state(base);
                for &s in symbols { self.affix_trans(base + (1 + s as usize)) }
                base
            })
    }

    /// Find the first index admitting all symbols.
    pub fn usher(&self, symbols : &[u8]) -> Option<usize> {
        self.as_state.iter()
            .find(|&&base|
                symbols.iter().all(|&s| self.as_trans.contains(&(base + (1 + s as usize)))))
            .cloned()
    }

    fn affix_state(&mut self, i : usize) {
        let r = self.as_state.remove(&i);
        assert!(r);
    }

    fn affix_trans(&mut self, i : usize) {
        let r = self.as_trans.remove(&i);
        assert!(r);
    }

    /// Add a new block to the segments.
    pub fn expand(&mut self, old_length : usize) {
        let new_length = old_length + self.block_size;
        self.as_state.extend(old_length .. new_length);
        self.as_trans.extend(old_length .. new_length);
    }

    pub fn block_size(&self) -> usize { self.block_size }

    pub fn unfixed_count(&self) -> usize {
        use std::cmp;
        cmp::min(self.as_trans.len(), self.as_state.len())
    }

    pub fn reserve(&mut self, n : usize) {
        self.as_state.reserve(n);
        self.as_trans.reserve(n);
    }
}

impl Default for IndexSegments {
    fn default() -> Self {
        IndexSegments {
            as_state : FnvHashSet::default(),
            as_trans : FnvHashSet::default(),
            block_size : 257,
        }
    }
}
