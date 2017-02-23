//! Paging structures for fast insertion in a DART.

/// The unit of a circular linked list for direct indexing of predecessor and
/// successor nodes. Primarily intended for the construction of static DARTs.
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct IndexLink {
    previous : usize,
    next : usize,
    fixed : bool
}

impl IndexLink {
    fn from_pair(previous : usize, next : usize) -> IndexLink {
        IndexLink {
            previous: previous,
            next: next,
            fixed: false
        }
    }
}


#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IndexSegments {
    /// The index of the first known vacancy.
    vacancy : Option<usize>,
    links : Vec<IndexLink>,
    block_size : usize
}

impl IndexSegments {
    /// Settle the transitions labelled with `symbols` in the segments,
    /// returning their base index.
    pub fn settle(&mut self, symbols : &[u8]) -> Option<usize> {
        self.usher(symbols).map(|base| {
            self.affix(base);
            for &s in symbols { self.affix(base + (1 + s as usize)) }
            base
        })
    }

    /// Find the first index admitting all symbols.
    pub fn usher(&self, symbols : &[u8]) -> Option<usize> {
        self.vacancy.and_then(|v| {
            let mut base = v;
            while false == self.admits(base, symbols) {
                base = self.links[base].next;
                // If we are back to the starting vacancy, we traversed all free indices
                // and found none compatible.
                if base == v { return None }
            }
            Some(base)
        })
    }

    /// Mark an index as fixed, meaning that it cannot be reused.
    pub fn affix(&mut self, i : usize) {
        assert!(self.links[i].fixed == false);

        self.abridge(i);
        self.links[i].fixed = true;
    }

    /// Whether the given index can accomodate all symbols.
    pub fn admits(&self, base : usize, symbols : &[u8]) -> bool {
        symbols.iter().all(|&s| self.admits_symbol(base, s))
    }

    fn admits_symbol(&self, base : usize, symbol : u8) -> bool {
        self.links.get(base + (1 + symbol as usize)).map_or(false, |l| !l.fixed)
    }

    /// Remove an index from the segments, mending the broken links.
    fn abridge(&mut self, i : usize) {
        let (prev, next) = (self.links[i].previous, self.links[i].next);

        self.links[next].previous = prev;
        self.links[prev].next = next;
        self.vacancy = match self.vacancy {
            Some(v) if v == i && i == next => None,
            Some(v) if v == i => Some(next),
            v@_ => v
        }
    }

    /// Add a new block to the segments and return the first vacancy.
    pub fn expand(&mut self) -> usize {
        let start = self.links.len();
        let end = start + self.block_size;
        let extension = (start .. end).map(|i| IndexLink::from_pair(i.saturating_sub(1), i + 1));
        self.links.extend(extension);

        match self.vacancy {
            Some(i) => {
                let empty_tail = self.links[i].previous;
                self.links[start].previous = empty_tail;
                self.links[empty_tail].next = start;
                self.links[i].previous = end - 1;
                self.links[end - 1].next = i;
            }
            None => {
                self.links[start].previous = end - 1;
                self.links[end - 1].next = start;
                self.vacancy = Some(start);
            }
        }

        self.vacancy.unwrap()
    }

    pub fn block_size(&self) -> usize { self.block_size }

    pub fn unfixed_count(&self) -> usize {
        self.links.iter().filter(|l| !l.fixed).count()
    }

    pub fn reserve(&mut self, n : usize) {
        self.links.reserve(n);
    }
}

impl Default for IndexSegments {
    fn default() -> Self {
        IndexSegments {
            vacancy : None,
            links : Vec::new(),
            block_size : 256
        }
    }
}
