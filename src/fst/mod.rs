pub mod builder;
pub mod error;
pub mod intermediate;
pub mod output;

use fnv::FnvHashMap;
use std::marker::PhantomData;

use fst::error::{Error, Result};
use fst::intermediate::Intermediary;
use fst::output::Output;
use index::Index;
use segment::IndexSegments;


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
    fn is(self) -> bool {
        match self {
            Terminal::Not => false,
            _ => true
        }
    }

    fn is_inner(self) -> bool {
        match self {
            Terminal::Inner => true,
            _ => false
        }
    }
}

impl Default for Terminal {
    fn default() -> Terminal { Terminal::Not }
}


#[cfg_attr(feature = "serialization", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FST<I, O> where I : Index, O : Output {
    pub da : Dart<I, O>,
    pub state_output : FnvHashMap<I, O>
}

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

    pub fn transition(&self, state : I, input : u8) -> Option<State<I>> {
        let e = state.as_usize() + (1 + input as usize);
        match self.da.stipe.get(e) {
            Some(&Stipe { check, terminal })
                if check == input => Some(State { index: self.da.next[e], terminal: terminal }),
            _ => None
        }
    }

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
