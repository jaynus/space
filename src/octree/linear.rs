use crate::{
    morton::{Morton, MortonMap, MortonRegionMap, MortonRegion, MortonWrapper, morton_levels},
    octree::Folder,
};

/// A linear hashed octree. This has constant time lookup for a given region or morton code.
///
/// ```
/// use space::{LinearOctree, Morton};
/// use nalgebra::Vector3;
///
/// let mut tree = LinearOctree::<String, u64>::new();
/// let coord = Vector3::<u64>::new(1, 2, 3);
///
/// // Insert a value into the tree
/// tree.insert(Morton::encode(coord), "test1".to_string() );
///
/// // Fetch a value at a specific coordinate
/// let fetched_value = tree.get(Morton::encode(coord));
/// assert_eq!("test1", *fetched_value.unwrap());
///
/// // Fetch a value that doesnt exist
/// let coord_empty = Vector3::<u64>::new(4, 5, 6);
/// let fetched_value = tree.get(Morton::encode(coord_empty));
/// assert!(fetched_value.is_none());
///
/// ```
#[derive(Clone)]
pub struct LinearOctree<T, M> {
    /// The leaves of the octree.
    leaves: MortonMap<T, M>,
    /// The each internal node either contains a `null` Morton or a non-null Morton which points to a leaf.
    /// Nodes which are not explicity stated implicitly indicate that it must be traversed deeper.
    internals: MortonRegionMap<M, M>,
}

impl<T, M> Default for LinearOctree<T, M>
    where
        M: Morton,
{
    /// Create a default, empty linear Octree
    ///
    /// ```
    /// use space::LinearOctree;
    /// let mut tree = LinearOctree::<String, u64>::new();
    ///
    /// ```
    fn default() -> Self {
        let mut internals = MortonRegionMap::default();
        internals.insert(MortonRegion::default(), M::null());
        Self {
            leaves: MortonMap::<_, M>::default(),
            internals,
        }
    }
}

impl<T, M> LinearOctree<T, M>
    where
        M: Morton,
{
    /// Create an empty octree. Calls Default impl.
    ///
    /// ```
    /// use space::LinearOctree;
    /// let tree = LinearOctree::<String, u64>::new();
    /// ```
    pub fn new() -> Self {
        Self::default()
    }

    /// Get iterator to the underlying MortonMap
    /// ```
    /// use space::{MortonWrapper, LinearOctree};
    /// let mut tree = LinearOctree::<String, u64>::new();
    /// let test_data = vec![
    ///     (1 as u64, "One".to_string()),
    ///     (2 as u64, "Two".to_string()),
    ///     (3 as u64, "Three".to_string())
    /// ];
    /// test_data.iter().for_each(|(m, v)| tree.insert(*m, v.clone()));
    ///
    /// for (m, v) in tree.iter() {
    ///     assert!(test_data.contains(&(m.0, v.clone())));
    /// }
    /// ```
    pub fn iter(&self) -> impl Iterator<Item = (&MortonWrapper<M>, &T)> {
        self.leaves.iter()
    }

    /// Get mutable iterator to the underlying MortonMap
    /// ```
    /// use space::{MortonWrapper, LinearOctree};
    /// let mut tree = LinearOctree::<String, u64>::new();
    /// let test_data = vec![
    ///     (1 as u64, "One".to_string()),
    ///     (2 as u64, "Two".to_string()),
    ///     (3 as u64, "Three".to_string())
    /// ];
    /// test_data.iter().for_each(|(m, v)| tree.insert(*m, v.clone()));
    ///
    /// for (m, mut v) in tree.iter_mut() {
    ///     assert!(test_data.contains(&(m.0, v.clone())));
    ///     *v = "balls".to_string();
    /// }
    /// for (m, v) in tree.iter() {
    ///     assert_eq!(v, "balls");
    /// }
    /// ```
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (&MortonWrapper<M>, &mut T)> {
        self.leaves.iter_mut()
    }


    /// Inserts the item into the octree.
    ///
    /// If another element occupied the exact same morton, it will be evicted and replaced.
    ///
    /// ```
    /// use space::{LinearOctree, Morton};
    /// use nalgebra::Vector3;
    ///
    /// let mut tree = LinearOctree::<String, u64>::new();
    /// tree.insert(Morton::encode(Vector3::new(1, 2, 3)), "test1".to_string() );
    ///
    /// ```
    pub fn insert(&mut self, morton: M, item: T) {
        use std::collections::hash_map::Entry::*;
        // First we must insert the node into the leaves.
        match self.leaves.entry(MortonWrapper(morton)) {
            Occupied(mut o) => {
                o.insert(item);
            }
            Vacant(v) => {
                v.insert(item);

                // Because it was vacant, we need to adjust the tree's internal nodes.
                for mut region in morton_levels(morton) {
                    // Check if the region is in the map.
                    if let Occupied(mut o) = self.internals.entry(region) {
                        // It was in the map. Check if it was null or not.
                        if o.get().is_null() {
                            // It was null, so just replace the null with the leaf.
                            *o.get_mut() = morton;
                            // Now return because we are done.
                            return;
                        } else {
                            // It was not null, so it is a leaf.
                            // This means that we need to move the leaf to its sub-region.
                            // We also need to populate the other 6 null nodes created by this operation.
                            let leaf = o.remove_entry().1;
                            // Keep making the tree deeper until both leaves differ.
                            // TODO: Some bittwiddling with mortons might be able to get the number of traversals.
                            for level in region.level..M::dim_bits() {
                                let leaf_level = leaf.get_level(level);
                                let item_level = morton.get_level(level);
                                if leaf_level == item_level {
                                    // They were the same so set every other region to null.
                                    for i in 0..8 {
                                        if i != leaf_level {
                                            self.internals.insert(region.enter(i), M::null());
                                        }
                                    }
                                    region = region.enter(leaf_level);
                                } else {
                                    // They were different, so set the other 6 regions null and make 2 leaves.
                                    for i in 0..8 {
                                        if i == leaf_level {
                                            self.internals.insert(region.enter(i), leaf);
                                        } else if i == item_level {
                                            self.internals.insert(region.enter(i), morton);
                                        } else {
                                            self.internals.insert(region.enter(i), M::null());
                                        }
                                    }
                                    // Now we must return as we have added the leaves.
                                    return;
                                }
                            }
                            unreachable!();
                        }
                    }
                }
            }
        }
    }

    /// Fetches an immutable reference to the value of a specific coordinate in the octree
    ///
    /// ```
    /// use space::{LinearOctree, Morton};
    /// use nalgebra::Vector3;
    ///
    /// let mut tree = LinearOctree::<String, u64>::new();
    ///
    /// let fetched_value = tree.get(Morton::encode(Vector3::<u64>::new(1, 2, 3)));
    /// assert!(fetched_value.is_none());
    /// ```
    pub fn get(&self, morton: M) -> Option<&T> {
        self.leaves.get(&MortonWrapper(morton))
    }

    /// Fetches a mutable reference to the value of a specific coordinate in the octree
    pub fn get_mut(&mut self, morton: M) -> Option<&mut T> {
        self.leaves.get_mut(&MortonWrapper(morton))
    }

    /// This gathers the octree in a tree fold by gathering leaves with `gatherer` and folding with `folder`.
    /// This allows information to be folded up the tree so it doesn't have to be computed multiple times.
    /// This has O(n) (exactly `n`) `gather` operations and O(n) (approximately `8/7 * n`) `fold` operations,
    /// with each gather operation always gathering `1` leaf and each `fold` operation gathering no more
    /// than `8` other fold sums.
    pub fn collect_fold<F>(&self, folder: &F) -> MortonRegionMap<F::Sum, M>
        where
            F: Folder<T, M>,
            F::Sum: Clone,
    {
        let mut map = MortonRegionMap::default();
        self.collect_fold_region(MortonRegion::base(), folder, &mut map);
        map
    }

    /// Same as `collect_fold`, but adds things to a morton region map and gives back the region.
    pub fn collect_fold_region<F>(
        &self,
        region: MortonRegion<M>,
        folder: &F,
        map: &mut MortonRegionMap<F::Sum, M>,
    ) -> Option<F::Sum>
        where
            F: Folder<T, M>,
            F::Sum: Clone,
    {
        match self.internals.get(&region) {
            Some(m) if !m.is_null() => {
                // This is a leaf node.
                let sum = folder.gather(*m, &self.leaves[&MortonWrapper(*m)]);
                map.insert(region, sum.clone());
                Some(sum)
            }
            None => {
                // This needs to be traversed deeper.
                let sum =
                    folder
                        .fold((0..8).filter_map(|i| {
                            self.collect_fold_region(region.enter(i), folder, map)
                        }));
                map.insert(region, sum.clone());
                Some(sum)
            }
            _ => None,
        }
    }
}

impl<T, M> Extend<(M, T)> for LinearOctree<T, M>
    where
        M: Morton + Default,
{
    fn extend<I>(&mut self, it: I)
        where
            I: IntoIterator<Item = (M, T)>,
    {
        for (morton, item) in it {
            self.insert(morton, item);
        }
    }
}
