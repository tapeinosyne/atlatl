// Forsaken docs justly quibble the vexed programmer's waning zeal.
extern crate fnv;
extern crate num_traits;
#[cfg(feature = "serde")]
#[macro_use] extern crate serde;

mod segment;
pub mod fst;
pub mod index;
