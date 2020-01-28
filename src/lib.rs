#![allow(dead_code)]

pub mod element;
pub mod table;

use crossbeam_epoch::pin;
use std::borrow::Borrow;
use std::cmp;
use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hash};
use std::sync::Arc;
use table::{do_hash, hash2idx, make_shift, Table};
use element::ElementReadGuard;

const TABLES_PER_MAP: usize = 2;

pub struct DashMap<K, V, S = RandomState> {
    tables: [Table<K, V, S>; TABLES_PER_MAP],
    hash_builder: Arc<S>,
    h2i_shift: usize,
}

impl<K: Eq + Hash, V> DashMap<K, V, RandomState> {
    pub fn new() -> Self {
        Self::with_hasher(RandomState::new())
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self::with_capacity_and_hasher(capacity, RandomState::new())
    }
}

impl<K: Eq + Hash, V, S: BuildHasher> DashMap<K, V, S> {
    fn yield_table<Q>(&self, key: &Q) -> (&Table<K, V, S>, u64)
    where
        K: Borrow<Q>,
        Q: ?Sized + Eq + Hash,
    {
        let hash = do_hash(&*self.hash_builder, key);
        let idx = hash2idx(hash, self.h2i_shift);
        (&self.tables[idx], hash)
    }

    pub fn with_hasher(hash_builder: S) -> Self {
        Self::with_capacity_and_hasher(0, hash_builder)
    }

    pub fn with_capacity_and_hasher(capacity: usize, hash_builder: S) -> Self {
        let hash_builder = Arc::new(hash_builder);
        let capacity_per_table = cmp::max(capacity, 4 * TABLES_PER_MAP) / TABLES_PER_MAP;
        let h2i_shift = make_shift(TABLES_PER_MAP);
        let table_iter =
            (0..TABLES_PER_MAP).map(|_| Table::new(capacity_per_table, hash_builder.clone()));
        let tables = array_init::from_iter(table_iter).unwrap();

        Self {
            tables,
            hash_builder,
            h2i_shift,
        }
    }

    pub fn batch<T>(&self, f: impl FnOnce(&Self) -> T) -> T {
        let guard = pin();
        let r = f(self);
        guard.defer(|| ());
        r
    }

    pub fn insert(&self, key: K, value: V) {
        let (table, hash) = self.yield_table(&key);
        table.insert(key, hash, value);
    }

    pub fn get<'a, Q>(&'a self, key: &Q) -> Option<ElementReadGuard<'a, K, V>>
    where
        K: Borrow<Q>,
        Q: ?Sized + Eq + Hash,
    {
        let (table, _) = self.yield_table(&key);
        table.get(key)
    }
}
