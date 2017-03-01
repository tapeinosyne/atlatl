use std::error;
use std::fmt;
use std::result;
use std::str;


pub type Result<T> = result::Result<T, Error>;

#[derive(Clone, Debug)]
pub enum Error {
    /// A duplicate key was inserted in the FST builder.
    Duplicate(Vec<u8>),
    /// A key was inserted out of order in the FST builder.
    OutOfOrder(Vec<u8>, Vec<u8>),
    /// The length of the Dart exceeds its index size.
    OutOfBounds { reached : usize, maximum : usize }
}

impl error::Error for Error {
    fn description(&self) -> &str {
        match *self {
            Error::Duplicate(_) => "a duplicate key was inserted in the FST builder",
            Error::OutOfOrder(_, _) => "a key was inserted out of order in the FST builder",
            Error::OutOfBounds { .. } => "the Dart has grown too large for its index type",
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Error::Duplicate(ref k) => write!(f, "\
FST construction error: the key
{}
was already present. All keys must be unique.", format_bytes(&k)),

            Error::OutOfOrder(ref k1, ref k2) => write!(f, "\
FST construction error: a key was inserted out of order.
The lesser key
{}
was inserted after the greater key
{}
Keys must be inserted in lexicographic order.", format_bytes(&k2), format_bytes(&k1)),

            Error::OutOfBounds { reached, maximum } => write!(f, "\
FST construction error: the FST outgrew its index type.
An FST with a maximum index of {} reached a state or transition that
required an index of {}.", maximum, reached),
        }
    }
}

fn format_bytes(bytes: &[u8]) -> String {
    match str::from_utf8(bytes) {
        Ok(s) => s.to_owned(),
        Err(_) => format!("{:?}", bytes),
    }
}
