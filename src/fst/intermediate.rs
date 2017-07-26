use fst::error::{Error, Result};
use fst::{FST, Output, Stipe, Terminal};
use fst::builder::{Builder, State};
use index::Index;
use segment::IndexSegments;


type BuilderState = usize;

#[derive(Clone, Debug, Default)]
pub struct Intermediary<I, O> where I : Index, O : Output {
    stack : Vec<BuilderState>,
    // Indexed by BuilderState
    registry : Vec<Option<I>>,
    segments : IndexSegments,
    fst : FST<I, O>
}

impl<I, O> Intermediary<I, O> where I : Index, O : Output {
    pub fn into_dart(self) -> FST<I, O> { self.fst }

    /// Build an intermediate representation
    pub fn from_builder(&mut self, fst : &Builder<I, O>) -> Result<()> {
        self.reserve(fst.size());
        self.registry.resize(fst.size(), None);
        let eph = &State::default();
        let mut states = vec![eph; fst.size()];
        for (state, &s_i) in fst.registry.iter() { states[s_i.as_usize()] = state }

        self.expand();
        let root_idx = fst.root().as_usize();
        let root_next = I::as_index(self.settle_root(states[root_idx]).unwrap());
        self.fst.da.next[0] = root_next;
        self.registry[root_idx] = Some(root_next);
        match (states[root_idx].terminal, states[root_idx].final_output) {
            (false, _) =>
                self.fst.da.stipe[0].terminal = Terminal::Not,
            (true, out) if out.is_zero() =>
                self.fst.da.stipe[0].terminal = Terminal::Empty,
            (true, out) => {
                self.fst.da.stipe[0].terminal = Terminal::Inner;
                self.fst.state_output.insert(I::zero(), out);
            }
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
                        let next = I::as_index( self.settle(&states[t]) ?);
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

        Ok(())
    }

    fn settle(&mut self, state : &State<I, O>) -> Result<usize> {
        let inputs : Vec<_> = state.transitions.iter().map(|t| t.label).collect();
        let base = self.first_available(&inputs);
        match base > I::bound() {
            true => Err(Error::OutOfBounds {
                reached : base,
                maximum : I::max_value().as_usize()
            }),
            false => Ok(base)
        }
    }

    fn settle_root(&mut self, state : &State<I, O>) -> Option<usize> {
        let inputs : Vec<_> = state.transitions.iter().map(|t| t.label).collect();
        self.expand();
        self.segments.settle_index(&inputs, 0)
    }

    fn first_available(&mut self, symbols : &[u8]) -> usize {
        self.segments.settle(symbols).or_else(|| {
            self.expand();
            self.segments.settle(symbols)
        }).unwrap()
    }

    fn expand(&mut self) {
        let old_length = self.len();
        self.fst.resize(old_length + self.segments.block_size());
        self.segments.expand(old_length);
    }

    fn reserve(&mut self, n : usize) {
        self.fst.reserve(n);
        self.segments.reserve(n / 64);
        self.registry.reserve(n);
    }

    pub fn len(&self) -> usize {
        self.fst.len()
    }

    pub fn unfixed_count(&self) -> usize {
        self.segments.unfixed_count()
    }
}
