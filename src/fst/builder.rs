use fnv::FnvHashMap;
use std::cmp;
use std::collections::hash_map::Entry;

use fst::error::{Error, Result};
use fst::output::Output;
use index::Index;


pub type Label = u8;

#[derive(Copy, Clone, Debug, Default, Hash, Eq, PartialEq)]
pub struct Transition<I, O> {
    pub label : Label,
    pub output : O,
    pub destination : I,
}

#[derive(Clone, Debug, Default, Hash, Eq, PartialEq)]
pub struct State<I, O> {
    pub terminal : bool,
    pub final_output : O,
    pub transitions : Vec<Transition<I, O>>
}


/// A transition without a fixed destination state.
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
struct DanglingArc<O> {
    label : Label,
    output : O
}

impl<O> DanglingArc<O> where O : Output {
    fn from_label(label : Label) -> DanglingArc<O> {
        DanglingArc { label, ..DanglingArc::default() }
    }
}


#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct DanglingState<I, O> {
    pub state : State<I, O>,
    pub last_arc : Option<DanglingArc<O>>
}

impl<I, O> DanglingState<I, O> where I : Index, O : Output {
    fn from_label(label : Label) -> DanglingState<I, O> {
        DanglingState {
            last_arc : Some(DanglingArc::from_label(label)),
            state : State::default()
        }
    }

    fn empty_terminal() -> DanglingState<I, O> {
        DanglingState {
            state : State { terminal : true, ..State::default() },
            last_arc : None
        }
    }

    fn affix_last(&mut self, destination : I) {
        if let Some(DanglingArc { label, output }) = self.last_arc.take() {
            self.state.transitions.push(Transition { destination, label, output });
        }
    }

    fn redistribute_output(&mut self, diff : O) {
        if diff != O::zero() {
            if self.state.terminal { self.state.final_output.mappend_assign(diff) }
            if let Some(ref mut t) = self.last_arc { t.output.mappend_assign(diff) }
            for t in &mut self.state.transitions { t.output.mappend_assign(diff) }
        }
    }
}


#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct DanglingPath<I, O> { stack : Vec<DanglingState<I, O>> }

impl<I, O> DanglingPath<I, O> where I : Index, O : Output {
    fn new() -> DanglingPath<I, O> {
        let mut dangling = DanglingPath { stack : Vec::with_capacity(64) };
        dangling.append_empty();
        dangling
    }

    fn append_empty(&mut self) {
        self.stack.push(DanglingState::default());
    }

    fn pop_empty(&mut self) -> State<I, O> {
        let dangling = self.stack.pop().unwrap();
        assert!(dangling.last_arc.is_none());
        dangling.state
    }

    fn pop_root(&mut self) -> State<I, O> {
        assert!(self.stack.len() == 1);
        assert!(self.stack[0].last_arc.is_none());
        self.stack.pop().unwrap().state
    }


    fn set_root_output(&mut self, output : O) {
        self.stack[0].state.terminal = true;
        self.stack[0].state.final_output = output;
    }

    fn finalize(&mut self, index : I) -> State<I, O> {
        let mut dangling = self.stack.pop().unwrap();
        dangling.affix_last(index);
        dangling.state
    }

    fn finalize_last(&mut self, index : I) {
        let last = self.stack.len() - 1;
        self.stack[last].affix_last(index);
    }

    fn add_suffix(&mut self, suffix : &[u8], output : O) {
        if suffix.is_empty() { return; }
        let last = self.stack.len() - 1;
        assert!(self.stack[last].last_arc.is_none());

        self.stack[last].last_arc = Some(DanglingArc { output, label : suffix[0] });
        self.stack.extend(suffix[1..].iter().map(|&l| DanglingState::from_label(l)));
        self.stack.push(DanglingState::empty_terminal());
    }

    fn redistribute_prefix(&mut self, key : &[u8], mut out : O) -> (usize, O) {
        let mut i = 0;
        while i < key.len() {
            let diff = match self.stack[i].last_arc.as_mut() {
                Some(ref mut t) if t.label == key[i] => {
                    i += 1;
                    let prefix = t.output.prefix(out);
                    let diff = t.output.inverse(prefix);
                    out.inverse_assign(prefix);
                    t.output = prefix;
                    diff
                }
                _ => break,
            };
            self.stack[i].redistribute_output(diff);
        }
        (i, out)
    }

    fn len(&self) -> usize { self.stack.len() }
}


type Registry<I, O> = FnvHashMap<State<I, O>, I>;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Builder<I, O> where I : Index, O : Output {
    pub registry : Registry<I, O>,
    dangling : DanglingPath<I, O>,
    previous_key : Option<Vec<u8>>,
    transition_count : usize,
    usable_index : usize,
    language_size : usize,
    root : I,
}

impl<I, O> Builder<I, O> where I : Index, O : Output {
    fn register(&mut self, state : State<I, O>) -> Result<I> {
        let idx = &mut self.usable_index;
        let trans_r = &mut self.transition_count;
        let trans_s = state.transitions.len();

        match self.registry.entry(state) {
            Entry::Occupied(e) => Ok(*e.get()),
            Entry::Vacant(e) => {
                let s_i = *idx;
                *idx += 1;
                *trans_r += trans_s;
                match s_i > I::bound() || *trans_r > I::bound() {
                    true => Err(Error::OutOfBounds {
                        reached : cmp::max(s_i, *trans_r),
                        maximum : I::max_value().as_usize()
                    }),
                    false => Ok(*e.insert(I::as_index(s_i)))
                }
            }
        }
    }

    fn finalize_subpath(&mut self, path_start : usize) -> Result<()> {
        let mut idx = None;
        while path_start + 1 < self.dangling.len() {
            let state = match idx {
                Some(i) => self.dangling.finalize(i),
                None => self.dangling.pop_empty()
            };
            idx = Some( self.register(state) ? );
        }
        // By construction, the last state remaining has no last_arc if `idx` is None
        if let Some(i) = idx { self.dangling.finalize_last(i) }
        Ok(())
    }

    fn finalize_root(&mut self) -> Result<I> {
        let root = self.dangling.pop_root();
        self.register(root)
    }

    fn validate_key<'a>(&mut self, key : &'a [u8]) -> Result<&'a [u8]> {
        match self.previous_key {
            Some(ref prev) if key == prev.as_slice() =>
                Err(Error::Duplicate(key.to_vec())),
            Some(ref prev) if key <  prev.as_slice() =>
                Err(Error::OutOfOrder(key.to_vec(), prev.to_vec())),
            _ => {
                self.previous_key = key.to_vec().into();
                Ok(key)
            }
        }
    }

    pub fn insert(&mut self, key : &[u8], value : O) -> Result<()> {
        let key = self.validate_key(key) ?;
        if key.is_empty() {
            self.dangling.set_root_output(value);
            self.language_size = 1;
            return Ok(());
        }
        let (prefix_len, output) = self.dangling.redistribute_prefix(key, value);
        self.finalize_subpath(prefix_len) ?;
        let suffix = &key[prefix_len ..];
        self.dangling.add_suffix(suffix, output);
        self.language_size += 1;
        Ok(())
    }

    pub fn finish(&mut self) -> Result<I> {
        self.finalize_subpath(0)
            .and_then(|_| self.finalize_root())
            .map(|i| {
                self.root = i;
                i
            })
    }

    pub fn from_iter<K, T>(iter : T) -> Result<Builder<I, O>>
        where K : AsRef<[u8]>
            , T : IntoIterator<Item = (K, O)>
    {
        let mut builder = Builder { dangling : DanglingPath::new(), ..Builder::default() };
        for (k, v) in iter { builder.insert(k.as_ref(), v) ? }
        builder.finish() ?;

        Ok(builder)
    }

    pub fn root(&self) -> I { self.root }

    pub fn size(&self) -> usize { self.registry.len() }

    pub fn len(&self) -> usize { self.language_size }
}
