// Forsaken docs justly quibble the vexed programmer's waning zeal.
extern crate fnv;
extern crate num_traits;
#[cfg(feature = "serialization")]
#[macro_use] extern crate serde_derive;

mod segment;
pub mod fst;
pub mod index;
