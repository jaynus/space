//! `space` intends to define the necessary abstractions and implementations for working with spatial data.
//!
//! Uses of spatial data structures:
//! - Point clouds
//! - k-NN (finding nearest neighborn in N-dimensional space)
//! - Collision detection
//! - N-body simulations
//!
//! This crate will not be 1.0 until it has removed all dependencies on nightly features and const generics
//! are available in stable to allow the abstraction over N-dimensional trees.
#![feature(box_syntax, box_patterns)]
//#![deny(missing_docs)]
#![deny(clippy::all, clippy::pedantic)]
#![allow(clippy::similar_names, clippy::module_name_repetitions)]

pub mod morton;
pub mod octree;

pub use morton::*;
pub use octree::*;

pub trait StorageAccess<'a, T: 'a, K> {
    type Iter: Iterator<Item=(K, &'a T)>;
    type IterMut: Iterator<Item=(K, &'a mut T)>;

    fn iter(&self, ) -> Self::Iter;
    fn iter_mut(&mut self, ) -> Self::IterMut;

    fn insert(&mut self, key: K, item: T);

    fn get(&mut self, key: K, item: T);
    fn get_mut(&mut self, key: K, item: T);
}

