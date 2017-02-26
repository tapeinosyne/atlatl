use std::hash::Hash;
use std::ops::{AddAssign, SubAssign};

use num_traits::Unsigned;


/// A minimal trait for unchecked casting of unsigned integers to `usize`,
/// for indexing purposes.
pub trait Index
    : Unsigned  // An unsigned integer—
    + Eq + Copy + Hash  // —with the properties we require—
    + Default + AddAssign + SubAssign  // —and a bit of convenience.
{
    fn as_usize(self) -> usize;
    fn as_index(i : usize) -> Self;
}

macro_rules! impl_index {
    ($idx:ty) => {
        impl Index for $idx {
            #[inline(always)]
            fn as_usize(self) -> usize { self as usize }

            #[inline(always)]
            fn as_index(i : usize) -> $idx { i as $idx }
        }
    }
}

impl_index! { usize }
impl_index! { u16   }
impl_index! { u32   }
#[cfg(target_pointer_width = "64")]
impl_index! { u64   }
