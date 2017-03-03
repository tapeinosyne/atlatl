pub mod builder;
pub mod error;
pub mod intermediate;
pub mod output;

pub use self::builder::Builder;
pub use self::error::Error;
pub use self::output::Output;


use fnv::FnvHashMap;
use std::slice;

use fst::error::Result;
use fst::intermediate::Intermediary;
use index::Index;


#[cfg_attr(feature = "serialization", derive(Serialize, Deserialize))]
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct Stipe {
    pub check : u8,
    pub terminal : Terminal
}

/// Finality of a transition's destination state.
#[cfg_attr(feature = "serialization", derive(Serialize, Deserialize))]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Terminal {
    /// The transition is not final.
    Not,
    /// The transition is final and leads to a state with no inner output.
    Empty,
    /// The transition is final and leads to a state with inner output.
    Inner
}

impl Terminal {
    #[inline] pub fn is(self) -> bool { self != Terminal::Not }
    #[inline] pub fn is_inner(self) -> bool { self == Terminal::Inner }
}

impl Default for Terminal {
    fn default() -> Terminal { Terminal::Not }
}


/// Hybrid Dart representation for a finite subsequential transducer.
#[cfg_attr(feature = "serialization", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FST<I, O> where I : Index, O : Output {
    pub da : Dart<I, O>,
    pub state_output : FnvHashMap<I, O>
}

/// The double-array trie, holding the core state machine for the FST.
#[cfg_attr(feature = "serialization", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Dart<I, O> {
    pub stipe : Vec<Stipe>,
    pub next : Vec<I>,
    pub output : Vec<O>,
}

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct State<I> { pub index : I, pub terminal : Terminal }

impl<I, O> FST<I, O> where I : Index, O : Output {
    pub fn from_builder(builder : &builder::Builder<I, O>) -> Result<Self> {
        let mut repr = Intermediary::default();
        repr.from_builder(builder) ?;
        Ok(repr.into_dart())
    }

    /// Given a starting state and an input, returns the destination state, if any.
    pub fn transition(&self, state : I, input : u8) -> Option<State<I>> {
        let e = state.as_usize() + (1 + input as usize);
        match self.da.stipe.get(e) {
            Some(&Stipe { check, terminal })
                if check == input => Some(State { index: self.da.next[e], terminal: terminal }),
            _ => None
        }
    }

    /// Returns whether the key is present in the FST.
    pub fn contains<K>(&self, key : K) -> bool
        where K : AsRef<[u8]>
    {
        let mut state = State::default();
        for &label in key.as_ref() {
            let to = self.transition(state.index, label);
            match to {
                Some(s) => state = s,
                _ => return false
            }
        }

        state.terminal.is()
    }

    /// Get the value associated to the key, if any.
    pub fn get<K>(&self, key : K) -> Option<O>
        where K : AsRef<[u8]>
    {
        let mut out = O::zero();
        let mut state = I::zero();
        let mut terminal = self.da.stipe[0].terminal;
        for &label in key.as_ref() {
            let e = state.as_usize() + (1 + label as usize);
            let stipe = self.da.stipe.get(e);
            match stipe {
                Some(stipe) if stipe.check == label => {
                    terminal = stipe.terminal;
                    out.mappend_assign(self.da.output[e]);
                    state = self.da.next[e];
                },
                _ => return None
            }
        }

        match terminal {
            Terminal::Not   => None,
            Terminal::Empty => Some(out),
            Terminal::Inner => Some(out.mappend(self.state_output[&state]))
        }
    }

    /// Returns an iterator producing the values associated to all prefixes
    /// of the query, including the empty string and the query itself.
    pub fn reap<'a, 'q>(&'a self, query : &'q [u8]) -> Reaper<'a, 'q, I, O> {
        let root_output = match self.da.stipe[0].terminal {
            Terminal::Not   => None,
            Terminal::Empty => Some((0, O::zero())),
            Terminal::Inner => Some((0, self.state_output[&I::zero()]))
        };

        Reaper {
            query : query.into_iter(),
            position : 0,
            fst : &self,
            root_output : root_output,
            output : O::zero(),
            state : I::zero()
        }
    }

    /// Returns an iterator producing the values associated to all prefixes
    /// of the query, including the query itself but excluding the empty string.
    pub fn reap_past_root<'a, 'q>(&'a self, query : &'q [u8]) -> RootlessReaper<'a, 'q, I, O> {
        RootlessReaper {
            query : query.into_iter(),
            position : 0,
            fst : &self,
            output : O::zero(),
            state : I::zero()
        }
    }

    /// The number of nodes in the internal double array, including surplus.
    pub fn len(&self) -> usize {
        assert!(self.da.next.len() == self.da.stipe.len());
        assert!(self.da.next.len() == self.da.output.len());
        self.da.stipe.len()
    }

    fn resize(&mut self, length : usize) {
        self.da.stipe.resize(length, Stipe::default());
        self.da.next.resize(length, I::zero());
        self.da.output.resize(length, O::zero());
    }

    fn reserve(&mut self, n : usize) {
        self.da.stipe.reserve(n);
        self.da.next.reserve(n);
        self.da.output.reserve(n);
    }
}


#[derive(Clone, Debug)]
pub struct Reaper<'a, 'q, I, O>
    where I : Index + 'a
        , O : Output + 'a
{
    query : slice::Iter<'q, u8>,
    position : usize,
    fst : &'a FST<I, O>,
    root_output : Option<(usize, O)>,
    state : I,
    output : O
}


// FIXME: the root-skipping doppelgänger of `Reaper` exists solely because
// downstream users of `atlatl` appear to suffer an inscrutable performance
// penalty if they do not avail themselves to both.
//
// Surely something is amiss.
#[derive(Clone, Debug)]
pub struct RootlessReaper<'a, 'q, I, O>
    where I : Index + 'a
        , O : Output + 'a
{
    query : slice::Iter<'q, u8>,
    position : usize,
    fst : &'a FST<I, O>,
    state : I,
    output : O
}

impl<'a, 'q, I, O> Iterator for RootlessReaper<'a, 'q, I, O>
    where I : Index, O : Output
{
    type Item = (usize, O);

    fn next(&mut self) -> Option<Self::Item> {
        let mut terminal = Terminal::Not;
        let da = &self.fst.da;
        for &label in self.query.by_ref() {
            let e = self.state.as_usize() + (1 + label as usize);
            let stipe = da.stipe.get(e);
            match stipe {
                Some(stipe) if stipe.check == label => {
                    self.output.mappend_assign(da.output[e]);
                    self.state = da.next[e];
                    self.position += 1;
                    terminal = stipe.terminal;
                    if terminal.is() { break }
                },
                _ => return None
            }
        }

        match terminal {
            Terminal::Not   => None,
            Terminal::Empty => Some((self.position, self.output)),
            Terminal::Inner =>
                Some((self.position,
                      self.output.mappend(self.fst.state_output[&self.state])))
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, Some(self.query.len()))
    }
}

impl<'a, 'q, I, O> Iterator for Reaper<'a, 'q, I, O>
    where I : Index, O : Output
{
    type Item = (usize, O);

    fn next(&mut self) -> Option<Self::Item> {
        // The root output representing the empty prefix, if present, is always
        // the first match of any query.
        self.root_output.take()
            .or_else(|| {
                let mut terminal = Terminal::Not;
                let da = &self.fst.da;
                for &label in self.query.by_ref() {
                    let e = self.state.as_usize() + (1 + label as usize);
                    let stipe = da.stipe.get(e);
                    match stipe {
                        Some(stipe) if stipe.check == label => {
                            self.output.mappend_assign(da.output[e]);
                            self.state = da.next[e];
                            self.position += 1;
                            terminal = stipe.terminal;
                            if terminal.is() { break }
                        },
                        _ => return None
                    }
                }

                match terminal {
                    Terminal::Not   => None,
                    Terminal::Empty => Some((self.position, self.output)),
                    Terminal::Inner =>
                        Some((self.position,
                              self.output.mappend(self.fst.state_output[&self.state])))
                }
            })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let from_root = if self.root_output.is_some() { 1 } else { 0 };
        (from_root, Some(self.query.len() + from_root))
    }
}
