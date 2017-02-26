use std::cmp;
use std::fmt::Debug;
use std::hash::Hash;


/// An additive abelian group with a prefix operation.
pub trait Output : Eq + Copy + Hash + Default + Debug {
    /// The identity element.
    fn zero() -> Self;

    /// The additive operation under which the output forms an abelian group.
    fn mappend(self, y : Self) -> Self;

    /// The additive operation applied to the inverse of `y`.
    fn inverse(self, y : Self) -> Self;

    /// The longest common prefix of the given values.
    fn prefix(self, y : Self) -> Self;

    #[inline] fn is_zero(self) -> bool { self == Self::zero() }

    #[inline] fn mappend_assign(&mut self, y : Self) { *self = self.mappend(y) }
    #[inline] fn inverse_assign(&mut self, y : Self) { *self = self.inverse(y) }
}

macro_rules! impl_output_unsigned {
    ($num:ty) => {
        impl Output for $num {
            #[inline] fn zero() -> Self { 0 }
            #[inline] fn mappend(self, y : Self) -> Self { self + y }
            #[inline] fn inverse(self, y : Self) -> Self { self - y }
            #[inline] fn prefix(self, y : Self) -> Self { cmp::min(self, y) }
        }
    }
}

macro_rules! impl_output_signed {
    ($num:ty) => {
        impl Output for $num {
            #[inline] fn zero() -> Self { 0 }
            #[inline] fn mappend(self, y : Self) -> Self { self + y }
            #[inline] fn inverse(self, y : Self) -> Self { self - y }

            #[inline]
            fn prefix(self, y : Self) -> Self {
                match (self > 0, y > 0) {
                    (true, true) => cmp::min(self, y),
                    (false, false) => cmp::max(self, y),
                    (_, _) => 0
                }
            }
        }
    }
}

impl_output_unsigned! { u8    }
impl_output_unsigned! { u16   }
impl_output_unsigned! { u32   }
impl_output_unsigned! { u64   }
impl_output_unsigned! { usize }

impl_output_signed! { i8    }
impl_output_signed! { i16   }
impl_output_signed! { i32   }
impl_output_signed! { i64   }
impl_output_signed! { isize }
