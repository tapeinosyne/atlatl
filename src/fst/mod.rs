pub mod builder;

use fnv::FnvHashMap;
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
pub struct FST {
    pub da : Dart,
    pub state_output : FnvHashMap<usize, u16>
}

#[cfg_attr(feature = "serialization", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Dart {
    pub stipe : Vec<Stipe>,
    pub next : Vec<usize>,
    pub output : Vec<u16>,
}

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct State { pub index : usize, pub terminal : Terminal }

impl FST {
    pub fn from_builder(builder : &builder::Builder) -> Self {
        let mut repr = Repr::default();
        repr.from_builder(builder);
        repr.into_dart()
    }

    pub fn transition(&self, state : usize, input : u8) -> Option<State> {
        let e = state + (1 + input as usize);
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

    pub fn get<K>(&self, key : K) -> Option<u16>
        where K : AsRef<[u8]>
    {
        let mut out = 0;
        let mut state = 0;
        let mut terminal = self.da.stipe[0].terminal;
        for &label in key.as_ref() {
            let e = state + (1 + label as usize);
            let stipe = self.da.stipe.get(e);
            match stipe {
                Some(stipe) if stipe.check == label => {
                    terminal = stipe.terminal;
                    out += self.da.output[e];
                    state = self.da.next[e];
                },
                _ => return None
            }
        }

        match terminal {
            Terminal::Not   => None,
            Terminal::Empty => Some(out),
            Terminal::Inner => Some(out + self.state_output[&state])
        }
    }

    pub fn len(&self) -> usize {
        assert!(self.da.next.len() == self.da.stipe.len());
        assert!(self.da.next.len() == self.da.output.len());
        self.da.stipe.len()
    }

    fn resize(&mut self, length : usize) {
        self.da.stipe.resize(length, Stipe::default());
        self.da.next.resize(length, 0);
        self.da.output.resize(length, 0);
    }

    fn reserve(&mut self, n : usize) {
        self.da.stipe.reserve(n);
        self.da.next.reserve(n);
        self.da.output.reserve(n);
    }
}


type DAState = usize;
type BuilderState = usize;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Repr {
    stack : Vec<BuilderState>,
    // Indexed by BuilderState
    registry : Vec<Option<DAState>>,
    segments : IndexSegments,
    // alphabet : Alphabet,
    fst : FST
}

impl Repr {
    pub fn into_dart(self) -> FST {
        self.fst
    }

    pub fn from_builder(&mut self, fst : &builder::Builder) {
        self.reserve(fst.size());
        self.registry.resize(fst.size(), None);
        let eph = &builder::State::default();
        let mut states = vec![eph; fst.size()];
        for (state, &s_i) in fst.registry.iter() { states[s_i] = state }

        self.expand();
        let root_idx = fst.root();
        let root_next = self.settle(states[root_idx]);
        self.fst.da.next[0] = root_next;
        self.registry[root_idx] = Some(root_next);
        if states[root_idx].terminal {
            self.fst.da.stipe[0].terminal = Terminal::Inner;
            self.fst.state_output.insert(0, states[root_idx].final_output);
        }

        self.stack.push(root_idx);
        while let Some(s_i) = self.stack.pop() {
            for trans in &states[s_i].transitions {
                let t = trans.destination;
                let (is_final, final_output) = (states[t].terminal, states[t].final_output);
                let terminal = match (is_final, final_output) {
                    (false, _) => Terminal::Not,
                    (true, 0) => Terminal::Empty,
                    (true, _) => Terminal::Inner
                };

                let label = trans.label;
                let e = self.registry[s_i].unwrap() + (1 + label as usize);
                if e >= self.fst.len() { self.expand(); }

                self.fst.da.output[e] = trans.output;
                self.fst.da.stipe[e] = Stipe { check: label, terminal: terminal };
                self.fst.da.next[e] = match self.registry[t] {
                    Some(i) => i,
                    None => {
                        let next = self.settle(&states[t]);
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

    fn settle(&mut self, state : &builder::State) -> usize {
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
