//! This module contains helpers to work with morton codes, otherwise known as a z-order curve.

mod region;
mod wrapper;

pub use self::region::*;
pub use self::wrapper::*;

use bitintr::{Pdep, Pext};
use nalgebra::Vector3;
use num_traits::{FromPrimitive, PrimInt, ToPrimitive};
use std::hash::{Hash, Hasher};

/// Use this to map regions defined by a z-order curve on a particular level to arbitrary objects.
/// This uses a custom hasher that is optimized for z-order data locality.
pub type MortonRegionMap<T, M> = std::collections::HashMap<MortonRegion<M>, T, MortonBuildHasher>;
/// Use this to have a set of regions defined by a z-order curve on a particular level.
/// This will not exclude subset regions.
/// This uses a custom hasher that is optimized for z-order data locality.
pub type MortonRegionSet<M> = std::collections::HashSet<MortonRegion<M>, MortonBuildHasher>;
/// Use this to map voxels in z-order to arbitrary objects.
/// This uses a custom hasher that is optimized for z-order data locality.
pub type MortonMap<T, M> = std::collections::HashMap<MortonWrapper<M>, T, MortonBuildHasher>;
/// Use this to keep a set of voxels in z-order.
/// This uses a custom hasher that is optimized for z-order data locality.
pub type MortonSet<M> = std::collections::HashSet<MortonWrapper<M>, MortonBuildHasher>;

/// Use this to map regions defined by a z-order curve on a particular level to arbitrary objects.
/// This uses a custom hasher that is optimized for z-order data locality.
/// This also uses an LRU cache under the hood so memory can be preserved.
pub type MortonRegionCache<T, M> = lru_cache::LruCache<MortonRegion<M>, T, MortonBuildHasher>;
/// Use this to map voxels in z-order to arbitrary objects.
/// This uses a custom hasher that is optimized for z-order data locality.
/// This also uses an LRU cache under the hood so memory can be preserved.
pub type MortonCache<T, M> = lru_cache::LruCache<MortonWrapper<M>, T, MortonBuildHasher>;

/// Create a `MortonRegionMap`.
pub fn region_map<T, M>() -> MortonRegionMap<T, M>
    where
        M: Morton,
{
    MortonRegionMap::default()
}

/// Create a `MortonRegionSet`.
pub fn region_set<M>() -> MortonRegionSet<M>
    where
        M: Morton,
{
    MortonRegionSet::default()
}

/// Create a `MortonMap`.
pub fn morton_map<T, M>() -> MortonMap<T, M>
    where
        M: Morton,
{
    MortonMap::default()
}

/// Create a `MortonSet`.
pub fn morton_set<T, M>() -> MortonSet<M>
    where
        M: Morton,
{
    MortonSet::default()
}

/// Create a `MortonRegionCache`.
pub fn region_cache<T, M>(size: usize) -> MortonRegionCache<T, M>
    where
        M: Morton,
{
    MortonRegionCache::with_hasher(size, MortonBuildHasher::default())
}

/// Create a `MortonCache`.
pub fn morton_cache<T, M>(size: usize) -> MortonCache<T, M>
    where
        M: Morton,
{
    MortonCache::with_hasher(size, MortonBuildHasher::default())
}

/// Invalidates pieces of a cache when something is changed at this particular morton.
pub fn invalidate_region_cache<T, M>(morton: M, cache: &mut MortonRegionCache<T, M>)
    where
        M: Morton,
{
    // Also remove the base region.
    cache.remove(&MortonRegion::base());
    for region in morton_levels(morton) {
        cache.remove(&region);
    }
}

/// Visits the values representing the difference, i.e. the keys that are in `primary` but not in `secondary`.
pub fn region_map_difference<'a, T, U, M>(
    primary: &'a MortonRegionMap<T, M>,
    secondary: &'a MortonRegionMap<U, M>,
) -> impl Iterator<Item = MortonRegion<M>> + 'a
    where
        M: Morton,
{
    primary.keys().filter_map(move |&k| {
        if secondary.get(&k).is_none() {
            Some(k)
        } else {
            None
        }
    })
}

/// Also known as a Z-order encoding, this partitions a bounded space into finite, but localized,
/// linear boxes. This morton code is always encoding 3 dimensional data.
pub trait Morton: PrimInt + FromPrimitive + ToPrimitive + Hash + std::fmt::Debug + 'static {
    /// This is the total number of bits in the primitive.
    const BITS: usize;

    /// Encode the three dimensions (x, y, z) into a morton code.
    fn encode(dims: Vector3<Self>) -> Self;
    /// Decode the morton code into the three individual dimensions (x, y, z).
    fn decode(self) -> Vector3<Self>;

    /// The number of bits used to represent each dimension.
    #[inline]
    fn dim_bits() -> usize {
        Self::BITS / 3
    }

    /// The highest level of the morton code's bits.
    #[inline]
    fn highest_bits() -> Self {
        Self::from_u8(0b111).unwrap() << (3 * (Self::dim_bits() - 1))
    }

    /// The bits in the morton that are used. Because there are three equal dimensions, that
    /// means that it will never perfectly divide into a power of two because a power of two, by definition,
    /// only has prime factors of 2, therefore regardless of the integer type there will always be 2 or 1 unsued
    /// bits that are not captured in the mask.
    #[inline]
    fn used_bits() -> Self {
        (Self::one() << (3 * Self::dim_bits())) - Self::one()
    }

    /// Same as `used_bits`, but its instead the mask of the bits not in use.
    #[inline]
    fn unused_bits() -> Self {
        !Self::used_bits()
    }

    /// Get the bits being used in a morton code with a particular level.
    ///
    /// If the level of a morton is 0, then we get only 3 bits from the "first" level.
    /// If the level of a morton is 1, then we get only 6 bits from the "first" and "second" levels.
    /// This continues until the level is the same as `Self::dim_bits() - 1`. This means this can only be
    /// called when `level` is in the range `[0, Self::dim_bits())`.
    #[inline]
    fn get_significant_bits(self, level: usize) -> Self {
        self >> (3 * (Self::dim_bits() - level - 1))
    }

    /// This is similar to `get_significant_bits`, but it also masks out all the levels above the specific
    /// one chosen so that a number from `[0, 8)` is returned, which allows the choosing of an octant at
    /// that `level`. By iterating over all the levels starting at `0`, it is possible to traverse an octree.
    #[inline]
    fn get_level(self, level: usize) -> usize {
        (self.get_significant_bits(level) & Self::from_u8(0b111).unwrap())
            .to_usize()
            .unwrap()
    }

    /// Gets the mask of a particular `level`.
    #[inline]
    fn level_mask(level: usize) -> Self {
        Self::highest_bits() >> (3 * level)
    }

    /// This will set the `level` of a morton code. The passed val must be in the range `[0, 8)`.
    #[inline]
    fn set_level(&mut self, level: usize, val: usize) {
        if Self::dim_bits() < level + 1 {
            panic!(
                "Morton::set_level: got invalid level {} (max is {})",
                level,
                Self::dim_bits() - 1
            );
        }
        self.reset_level(level);
        *self = *self | Self::from_usize(val).unwrap() << (3 * (Self::dim_bits() - level - 1))
    }

    /// This sets a particular `level` in a morton code to `0`.
    #[inline]
    fn reset_level(&mut self, level: usize) {
        *self = *self & !Self::level_mask(level)
    }

    /// Because the upper bits are never set in the morton code, it is possible to create a unique morton code
    /// that doesn't represent an actual place in an octree which can be used as a null morton code.
    #[inline]
    fn null() -> Self {
        !Self::zero()
    }

    /// This checks if a morton code is the null code obtained from `Self::null()`.
    #[inline]
    fn is_null(self) -> bool {
        self == Self::null()
    }
}

impl Morton for u64 {
    const BITS: usize = 64;

    /// Encode a Vector3<u64> into a morton code.
    ///
    /// ```
    /// use space::Morton;
    /// use nalgebra::Vector3;
    ///
    /// let morton_code = Morton::encode(Vector3::<u64>::new(1, 2, 3));
    /// assert_eq!(morton_code, 53);
    /// ```
    #[inline]
    fn encode(dims: Vector3<Self>) -> Self {
        let [x, y, z]: [Self; 3] = dims.into();
        let bits = 0x1_249_249_249_249_249_u64;
        z.pdep(bits << 2) | y.pdep(bits << 1) | x.pdep(bits)
    }

    /// Decode a u64 morton to its associated Vector3<u64>
    ///
    /// ```
    /// use space::Morton;
    /// use nalgebra::Vector3;
    ///
    /// let coordinates = Morton::decode(53);
    /// assert_eq!(coordinates, Vector3::<u64>::new(1, 2, 3));
    /// ```
    #[inline]
    fn decode(self) -> Vector3<Self> {
        let bits = 0x1_249_249_249_249_249_u64;
        let (x, y, z) = (self.pext(bits), self.pext(bits << 1), self.pext(bits << 2));
        Vector3::new(x & Self::used_bits(), y, z)
    }
}

impl Morton for u128 {
    const BITS: usize = 128;

    /// Encode a Vector3<u128> into a morton code.
    ///
    /// ```
    /// use space::Morton;
    /// use nalgebra::Vector3;
    ///
    /// let morton_code = Morton::encode(Vector3::<u128>::new(1, 2, 3));
    /// assert_eq!(morton_code, 53);
    /// ```
    #[inline]
    #[allow(clippy::cast_lossless, clippy::cast_possible_truncation)]
    fn encode(dims: Vector3<Self>) -> Self {
        let [x, y, z]: [Self; 3] = dims.into();
        let highx = (x >> 21) & ((1 << 21) - 1);
        let lowx = x & ((1 << 21) - 1);
        let highy = (y >> 21) & ((1 << 21) - 1);
        let lowy = y & ((1 << 21) - 1);
        let highz = (z >> 21) & ((1 << 21) - 1);
        let lowz = z & ((1 << 21) - 1);
        let high = u64::encode([highx as u64, highy as u64, highz as u64].into());
        let low = u64::encode([lowx as u64, lowy as u64, lowz as u64].into());
        (high as Self) << 63 | low as Self
    }

    /// Decode a u128 morton to its associated Vector3<u128>
    ///
    /// ```
    /// use space::Morton;
    /// use nalgebra::Vector3;
    ///u128
    /// let coordinates = Morton::decode(53);
    /// assert_eq!(coordinates, Vector3::<u128>::new(1, 2, 3));
    /// ```
    #[inline]
    #[allow(clippy::cast_lossless, clippy::cast_possible_truncation)]
    fn decode(self) -> Vector3<Self> {
        let low = self as u64;
        let high = (self >> 63) as u64;
        let [lowx, lowy, lowz]: [u64; 3] = low.decode().into();
        let [highx, highy, highz]: [u64; 3] = high.decode().into();
        Vector3::new(
            (highx << 21 | lowx) as Self,
            (highy << 21 | lowy) as Self,
            (highz << 21 | lowz) as Self,
        )
    }
}

/// The `BuildHasher` for `MortonHash`.
pub type MortonBuildHasher = std::hash::BuildHasherDefault<MortonHash>;

/// This const determines how many significant bits from the morton get added into the hash instead of multiplied
/// by the FNV prime. This is done to improve cache locality for mortons and works to great effect. Unfortunately,
/// this has a slight impact on memory consumption a small amount that depends on the dataset, but the performance
/// is drastically better for local interactions due to cache locality. Little is gained by going to higher amounts
/// of bits than `3` and the memory cost is too high, so this is currently hardcoded to `3`.
const CACHE_LOCALITY_BITS: usize = 3;

/// This is not to be used with anything other than a morton code, as it depends on its unique structure.
/// It is safe to use it with other data, but it wont perform well at all and may eat tons of memory.
/// Use at your own risk.
#[derive(Copy, Clone, Default)]
pub struct MortonHash {
    value: u64,
}

#[allow(clippy::cast_lossless)]
impl Hasher for MortonHash {
    #[inline]
    fn finish(&self) -> u64 {
        self.value
    }

    ///```#[should_panic]
    /// use std::hash::Hasher;
    /// use space::MortonHash;
    ///
    /// let mut hash = MortonHash::default();
    /// let val = [0; 1];
    /// hash.write(&val);
    ///```
    #[inline]
    fn write(&mut self, _: &[u8]) {
        panic!("Morton hash should only be used with a single 64 bit value");
    }

    ///```#[should_panic]
    /// use std::hash::Hasher;
    /// use space::MortonHash;
    ///
    /// let mut hash = MortonHash::default();
    /// hash.write_u8(1);
    ///```
    fn write_u8(&mut self, _: u8) {
        panic!("Morton hash should only be used with a single 64 bit value");
    }

    ///```#[should_panic]
    /// use std::hash::Hasher;
    /// use space::MortonHash;
    ///
    /// let mut hash = MortonHash::default();
    /// hash.write_u16(1);
    ///```
    fn write_u16(&mut self, _: u16) {
        panic!("Morton hash should only be used with a single 64 bit value");
    }

    ///```#[should_panic]
    /// use std::hash::Hasher;
    /// use space::MortonHash;
    ///
    /// let mut hash = MortonHash::default();
    /// hash.write_u32(1);
    ///```
    fn write_u32(&mut self, _: u32) {
        panic!("Morton hash should only be used with a single 64 bit value");
    }

    ///```
    /// use std::hash::Hasher;
    /// use space::MortonHash;
    ///
    /// let mut hash = MortonHash::default();
    /// hash.write_u64(123);
    /// assert_eq!(hash.finish(), 12638158613253308507);
    ///```
    #[inline(always)]
    #[allow(clippy::unreadable_literal)]
    fn write_u64(&mut self, i: u64) {
        let bottom_mask = (1 << CACHE_LOCALITY_BITS) - 1;
        let bottom = i & bottom_mask;
        let top = (i & !bottom_mask) >> CACHE_LOCALITY_BITS;
        self.value =
            ((top ^ 14695981039346656037).wrapping_mul(1099511628211) & !bottom_mask) + bottom;
    }

    ///```
    /// use std::hash::Hasher;
    /// use space::MortonHash;
    ///
    /// let mut hash = MortonHash::default();
    /// hash.write_u128(123);
    /// assert_eq!(hash.finish(), 12638158613253308507);
    ///```
    #[inline(always)]
    #[allow(clippy::unreadable_literal, clippy::cast_lossless, clippy::cast_possible_truncation)]
    fn write_u128(&mut self, i: u128) {
        let bottom_mask = (1 << CACHE_LOCALITY_BITS) - 1;
        let bottom = i & bottom_mask;
        let top = (i & !bottom_mask) >> CACHE_LOCALITY_BITS;
        self.value = (((top ^ 14695981039346656037).wrapping_mul(1099511628211) & !bottom_mask)
            + bottom) as u64;
    }

    ///```#[should_panic]
    /// use std::hash::Hasher;
    /// use space::MortonHash;
    ///
    /// let mut hash = MortonHash::default();
    /// hash.write_usize(123);
    ///```
    fn write_usize(&mut self, _: usize) {
        panic!("Morton hash should only be used with a single 64 bit value");
    }

    ///```#[should_panic]
    /// use std::hash::Hasher;
    /// use space::MortonHash;
    ///
    /// let mut hash = MortonHash::default();
    /// hash.write_i8(123);
    ///```
    fn write_i8(&mut self, _: i8) {
        panic!("Morton hash should only be used with a single 64 bit value");
    }

    ///```#[should_panic]
    /// use std::hash::Hasher;
    /// use space::MortonHash;
    ///
    /// let mut hash = MortonHash::default();
    /// hash.write_i16(123);
    ///```
    fn write_i16(&mut self, _: i16) {
        panic!("Morton hash should only be used with a single 64 bit value");
    }

    ///```#[should_panic]
    /// use std::hash::Hasher;
    /// use space::MortonHash;
    ///
    /// let mut hash = MortonHash::default();
    /// hash.write_i32(123);
    ///```
    fn write_i32(&mut self, _: i32) {
        panic!("Morton hash should only be used with a single 64 bit value");
    }

    ///```#[should_panic]
    /// use std::hash::Hasher;
    /// use space::MortonHash;
    ///
    /// let mut hash = MortonHash::default();
    /// hash.write_i64(123);
    ///```
    fn write_i64(&mut self, _: i64) {
        panic!("Morton hash should only be used with a single 64 bit value");
    }

    ///```#[should_panic]
    /// use std::hash::Hasher;
    /// use space::MortonHash;
    ///
    /// let mut hash = MortonHash::default();
    /// hash.write_i128(123);
    ///```
    fn write_i128(&mut self, _: i128) {
        panic!("Morton hash should only be used with a single 64 bit value");
    }

    ///```#[should_panic]
    /// use std::hash::Hasher;
    /// use space::MortonHash;
    ///
    /// let mut hash = MortonHash::default();
    /// hash.write_isize(123);
    ///```
    fn write_isize(&mut self, _: isize) {
        panic!("Morton hash should only be used with a single 64 bit value");
    }
}

#[test]
fn test_write() {
    use crate::MortonHash;
    use std::hash::Hasher;

    let mut hash = MortonHash::default();
    hash.write_u64(123);
    println!("hash={}", hash.finish());
}