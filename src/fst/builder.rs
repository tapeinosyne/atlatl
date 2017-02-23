use fnv::FnvHashMap;


pub type Index = usize; // to be made generic
pub type Label = u8;

#[derive(Copy, Clone, Debug, Default, Hash, Eq, PartialEq)]
pub struct Transition {
    pub label : Label,
    pub output : u16,   // to be made generic
    pub destination : Index,
}

#[derive(Clone, Debug, Default, Hash, Eq, PartialEq)]
pub struct State {
    pub terminal : bool,
    pub final_output : u16,
    pub transitions : Vec<Transition>
}


/// A transition without a fixed destination state.
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
struct DanglingArc {
    label : Label,
    output : u16
}

impl DanglingArc {
    fn from_label(label : Label) -> DanglingArc {
        DanglingArc {
            label : label,
            ..DanglingArc::default()
        }
    }

    fn from_pair(label : Label, output : u16) -> DanglingArc {
        DanglingArc {
            label : label,
            output : output
        }
    }
}


#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct DanglingState {
    pub state : State,
    pub last_arc : Option<DanglingArc>
}

impl DanglingState {
    fn from_label(label : Label) -> DanglingState {
        DanglingState {
            last_arc : Some(DanglingArc::from_label(label)),
            state : State::default()
        }
    }

    fn empty_terminal() -> DanglingState {
        DanglingState {
            state : State { terminal : true, ..State::default() },
            last_arc : None
        }
    }

    fn affix_last(&mut self, index : Index) {
        if let Some(arc) = self.last_arc.take() {
            self.state.transitions.push(Transition {
                label : arc.label,
                output : arc.output,
                destination : index,
            });
        }
    }

    fn redistribute_output(&mut self, diff : u16) {
        if diff != 0 {
            if self.state.terminal { self.state.final_output += diff }
            if let Some(ref mut t) = self.last_arc { t.output += diff }
            for t in &mut self.state.transitions { t.output += diff }
        }
    }
}


#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct DanglingPath { stack : Vec<DanglingState> }

impl DanglingPath {
    fn new() -> DanglingPath {
        let mut dangling = DanglingPath { stack : Vec::with_capacity(64) };
        dangling.append_empty();
        dangling
    }

    fn append_empty(&mut self) {
        self.stack.push(DanglingState::default());
    }

    fn pop_empty(&mut self) -> State {
        let dangling = self.stack.pop().unwrap();
        assert!(dangling.last_arc.is_none());
        dangling.state
    }

    fn pop_root(&mut self) -> State {
        assert!(self.stack.len() == 1);
        assert!(self.stack[0].last_arc.is_none());
        self.stack.pop().unwrap().state
    }


    fn set_root_output(&mut self, output : u16) {
        self.stack[0].state.terminal = true;
        self.stack[0].state.final_output = output;
    }

    fn finalize(&mut self, index : Index) -> State {
        let mut dangling = self.stack.pop().unwrap();
        dangling.affix_last(index);
        dangling.state
    }

    fn finalize_last(&mut self, index : Index) {
        let last = self.stack.len() - 1;
        self.stack[last].affix_last(index);
    }

    fn add_suffix(&mut self, suffix : &[u8], output : u16) {
        if suffix.is_empty() { return; }
        let last = self.stack.len() - 1;
        assert!(self.stack[last].last_arc.is_none());

        self.stack[last].last_arc = Some(DanglingArc::from_pair(suffix[0], output));
        self.stack.extend(suffix[1..].iter().map(|&l| DanglingState::from_label(l)));
        self.stack.push(DanglingState::empty_terminal());
    }

    fn redistribute_prefix(&mut self, key : &[u8], mut out : u16) -> (usize, u16) {
        use std::cmp;
        let mut i = 0;
        while i < key.len() {
            let diff = match self.stack[i].last_arc.as_mut() {
                Some(ref mut t) if t.label == key[i] => {
                    i += 1;
                    let prefix = cmp::min(t.output, out);
                    let diff = t.output - prefix;
                    out -= prefix;
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


type Registry = FnvHashMap<State, Index>;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Builder {
    pub registry : Registry,
    dangling : DanglingPath,
    previous_key : Option<Vec<u8>>,
    usable_index : Index,
    language_size : usize,
    root : Index,
}

impl Builder {
    fn register(&mut self, state : State) -> Index {
        let idx = &mut self.usable_index;
        * self.registry.entry(state).or_insert_with(|| {
            let i = *idx;
            *idx += 1;
            i
        })
    }

    fn finalize_subpath(&mut self, path_start : usize) {
        let mut idx = None;
        while path_start + 1 < self.dangling.len() {
            let state = match idx {
                Some(i) => self.dangling.finalize(i),
                None => self.dangling.pop_empty()
            };
            idx = self.register(state).into();
        }
        // By construction, the last state remaining has no last_arc if `idx` is None
        if let Some(i) = idx { self.dangling.finalize_last(i) }
    }

    fn finalize_root(&mut self) -> Index {
        let root = self.dangling.pop_root();
        self.register(root)
    }

    fn check_key(&mut self, key : &[u8]) {
        match self.previous_key {
            Some(ref prev) if key == prev.as_slice() =>
                panic!("Duplicate key: {:?}", key),
            Some(ref prev) if key <  prev.as_slice() =>
                panic!("Out of order: {:?}, {:?}", key, prev),
            _ => self.previous_key = key.to_vec().into()
        }
    }

    pub fn insert(&mut self, key : &[u8], value : u16) {
        self.check_key(key);
        if key.is_empty() {
            self.dangling.set_root_output(value);
            self.language_size = 1;
            return;
        }
        let (prefix_len, output) = self.dangling.redistribute_prefix(key, value);
        self.finalize_subpath(prefix_len);
        let suffix = &key[prefix_len ..];
        self.dangling.add_suffix(suffix, output);
        self.language_size += 1;
    }

    pub fn finish(&mut self) -> Index {
        self.finalize_subpath(0);
        let root_idx = self.finalize_root();
        self.root = root_idx;
        root_idx
    }

    pub fn from_iter<K, I>(iter : I) -> Builder
        where K : AsRef<[u8]>
            , I : IntoIterator<Item = (K, u16)>
    {
        let mut builder = Builder { dangling : DanglingPath::new(), ..Builder::default() };
        for (k, v) in iter { builder.insert(k.as_ref(), v) }
        builder.finish();

        builder
    }

    pub fn root(&self) -> Index { self.root }

    pub fn size(&self) -> usize { self.registry.len() }

    pub fn len(&self) -> usize { self.language_size }
}
