pub mod builder;
pub mod output;

use fnv::FnvHashMap;
use segment::IndexSegments;

use index::Index;
use fst::output::Output;


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
    pub fn from_builder(builder : &builder::Builder<I, O>) -> Self {
        let mut repr = Repr::default();
        repr.from_builder(builder);
        repr.into_dart()
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


type BuilderState = usize;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Repr<I, O> where I : Index, O : Output {
    stack : Vec<BuilderState>,
    // Indexed by BuilderState
    registry : Vec<Option<I>>,
    segments : IndexSegments,
    // alphabet : Alphabet,
    fst : FST<I, O>
}

impl<I, O> Repr<I, O> where I : Index, O : Output {
    pub fn into_dart(self) -> FST<I, O> {
        self.fst
    }

    pub fn from_builder(&mut self, fst : &builder::Builder<I, O>) {
        self.reserve(fst.size());
        self.registry.resize(fst.size(), None);
        let eph = &builder::State::default();
        let mut states = vec![eph; fst.size()];
        for (state, &s_i) in fst.registry.iter() { states[s_i.as_usize()] = state }

        self.expand();
        let root_idx = fst.root().as_usize();
        let root_next = I::as_index(self.settle(states[root_idx]));
        self.fst.da.next[0] = root_next;
        self.registry[root_idx] = Some(root_next);
        if states[root_idx].terminal {
            self.fst.da.stipe[0].terminal = Terminal::Inner;
            self.fst.state_output.insert(I::zero(), states[root_idx].final_output);
        }

        self.stack.push(root_idx);
        while let Some(s_i) = self.stack.pop() {
            for trans in &states[s_i].transitions {
                let t = trans.destination.as_usize();
                let (is_final, final_output) = (states[t].terminal, states[t].final_output);
                let terminal = match (is_final, final_output.is_zero()) {
                    (false, _) => Terminal::Not,
                    (true, true) => Terminal::Empty,
                    (true, false) => Terminal::Inner
                };

                let label = trans.label;
                let e = self.registry[s_i].unwrap().as_usize() + (1 + label as usize);
                if e >= self.fst.len() { self.expand(); }

                self.fst.da.output[e] = trans.output;
                self.fst.da.stipe[e] = Stipe { check: label, terminal: terminal };
                self.fst.da.next[e] = match self.registry[t] {
                    Some(i) => i,
                    None => {
                        let next = I::as_index(self.settle(&states[t]));
                        self.registry[t] = Some(next);
                        self.stack.push(t);
                        if terminal.is_inner() {
                            self.fst.state_output.insert(next, final_output);
                        }
                        next
                    }
                };
            }
        }
    }

    fn settle(&mut self, state : &builder::State<I, O>) -> usize {
        let inputs : Vec<_> = state.transitions.iter().map(|t| t.label).collect();
        self.first_available(&inputs)
    }

    fn first_available(&mut self, symbols : &[u8]) -> usize {
        self.segments.settle(symbols).or_else(|| {
            self.expand();
            self.segments.settle(symbols)
        }).unwrap()
    }

    fn expand(&mut self) -> usize {
        let old_length = self.len();
        self.fst.resize(old_length + 256);
        self.segments.expand()
    }

    fn reserve(&mut self, n : usize) {
        self.fst.reserve(n);
        self.segments.reserve(n);
        self.registry.reserve(n);
    }

    pub fn len(&self) -> usize {
        self.fst.len()
    }

    pub fn unfixed_count(&self) -> usize {
        self.segments.unfixed_count()
    }
}
