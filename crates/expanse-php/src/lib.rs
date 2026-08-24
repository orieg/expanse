#![allow(missing_docs)]

//! Native PHP Zend Engine extension for Expanse using `ext-php-rs`.
//!
//! Provides zero-overhead opcode execution and direct Judy compatibility in PHP 8.1+.

use expanse_trie::bytesmap::ExpanseBytesMap;
use expanse_trie::strmap::ExpanseStrMap;
use expanse_trie::sync::{SyncExpanseMap, SyncExpanseSet};
use expanse_trie::{ExpanseBlobMap, ExpanseMap, ExpanseSet};
use ext_php_rs::prelude::*;

/// Native Expanse Set (`Expanse\ExpanseSet`)
#[php_class]
#[php(name = "Expanse\\ExpanseSet")]
#[derive(Default)]
pub struct PhpExpanseSet {
    inner: ExpanseSet,
}

#[php_impl]
impl PhpExpanseSet {
    /// Creates a new empty ExpanseSet.
    pub fn __construct() -> Self {
        Self::default()
    }

    /// Adds a 64-bit integer key to the set. Returns true if newly inserted.
    pub fn add(&mut self, key: u64) -> bool {
        self.inner.insert(key)
    }

    /// Removes a 64-bit integer key from the set. Returns true if removed.
    pub fn remove(&mut self, key: u64) -> bool {
        self.inner.remove(key)
    }

    /// Checks if a 64-bit integer key exists in the set.
    pub fn contains(&self, key: u64) -> bool {
        self.inner.contains(key)
    }

    /// Returns the total population of keys in the set.
    pub fn count(&self) -> usize {
        self.inner.len() as usize
    }

    /// Clears all keys from the set.
    pub fn clear(&mut self) {
        self.inner.clear();
    }

    /// Returns the smallest key in the set, or null if empty.
    pub fn first(&self) -> Option<u64> {
        self.inner.first()
    }

    /// Returns the largest key in the set, or null if empty.
    pub fn last(&self) -> Option<u64> {
        self.inner.last()
    }

    /// Returns the smallest key strictly greater than `key`, or null if none.
    pub fn next(&self, key: u64) -> Option<u64> {
        self.inner.next_after(key)
    }

    /// Returns the greatest key strictly less than `key`, or null if none.
    pub fn prev(&self, key: u64) -> Option<u64> {
        self.inner.prev_before(key)
    }

    /// Returns the rank (number of keys strictly less than `key`) in O(depth).
    pub fn rank(&self, key: u64) -> u64 {
        self.inner.count_below(key)
    }

    /// Returns the n-th key (0-indexed) in sorted order in O(depth).
    pub fn select(&self, index: u64) -> Option<u64> {
        self.inner.by_count(index)
    }

    /// Returns the count of keys in the range `[start, end]`.
    #[php(name = "countRange")]
    pub fn count_range(&self, start: u64, end: u64) -> u64 {
        self.inner.count_range(start..=end)
    }

    /// Returns the exact memory used by the set in bytes.
    #[php(name = "memUsed")]
    pub fn mem_used(&self) -> usize {
        self.inner.mem_used()
    }
}

/// Native Expanse Map (`Expanse\ExpanseMap`)
#[php_class]
#[php(name = "Expanse\\ExpanseMap")]
#[derive(Default)]
pub struct PhpExpanseMap {
    inner: ExpanseMap,
}

#[php_impl]
impl PhpExpanseMap {
    /// Creates a new empty ExpanseMap.
    pub fn __construct() -> Self {
        Self::default()
    }

    /// Sets key -> value.
    pub fn set(&mut self, key: u64, value: u64) {
        self.inner.insert(key, value);
    }

    /// Retrieves the 64-bit value associated with key, or null if absent.
    pub fn get(&self, key: u64) -> Option<u64> {
        self.inner.get(key)
    }

    /// Deletes a key from the map. Returns true if removed.
    pub fn delete(&mut self, key: u64) -> bool {
        self.inner.remove(key).is_some()
    }

    /// Checks if a key exists in the map.
    pub fn has(&self, key: u64) -> bool {
        self.inner.contains_key(key)
    }

    /// Returns the total number of entries in the map.
    pub fn count(&self) -> usize {
        self.inner.len() as usize
    }

    /// Clears all entries from the map.
    pub fn clear(&mut self) {
        self.inner.clear();
    }

    /// Returns [key, value] for the smallest key in the map, or null if empty.
    pub fn first(&self) -> Option<Vec<u64>> {
        self.inner.first().map(|(k, v)| vec![k, v])
    }

    /// Returns [key, value] for the largest key in the map, or null if empty.
    pub fn last(&self) -> Option<Vec<u64>> {
        self.inner.last().map(|(k, v)| vec![k, v])
    }

    /// Returns [key, value] for the smallest key strictly greater than `key`, or null if none.
    pub fn next(&self, key: u64) -> Option<Vec<u64>> {
        self.inner.next_after(key).map(|(k, v)| vec![k, v])
    }

    /// Returns [key, value] for the greatest key strictly less than `key`, or null if none.
    pub fn prev(&self, key: u64) -> Option<Vec<u64>> {
        self.inner.prev_before(key).map(|(k, v)| vec![k, v])
    }

    /// Returns the rank (number of keys strictly less than `key`) in O(depth).
    pub fn rank(&self, key: u64) -> u64 {
        self.inner.count_below(key)
    }

    /// Returns [key, value] for the n-th entry (0-indexed) in sorted order in O(depth).
    pub fn select(&self, index: u64) -> Option<Vec<u64>> {
        self.inner.by_count(index).map(|(k, v)| vec![k, v])
    }

    /// Returns the count of keys in the range `[start, end]`.
    #[php(name = "countRange")]
    pub fn count_range(&self, start: u64, end: u64) -> u64 {
        self.inner.count_range(start..=end)
    }

    /// Returns the exact memory used by the map in bytes.
    #[php(name = "memUsed")]
    pub fn mem_used(&self) -> usize {
        self.inner.mem_used()
    }
}

/// Native Expanse String Map (`Expanse\ExpanseStrMap`)
#[php_class]
#[php(name = "Expanse\\ExpanseStrMap")]
#[derive(Default)]
pub struct PhpExpanseStrMap {
    inner: ExpanseStrMap,
}

#[php_impl]
impl PhpExpanseStrMap {
    /// Creates a new empty ExpanseStrMap.
    pub fn __construct() -> Self {
        Self::default()
    }

    /// Sets string key -> integer value.
    pub fn set(&mut self, key: &str, value: u64) {
        self.inner.insert(key.as_bytes(), value);
    }

    /// Gets the integer value associated with key, or null if absent.
    pub fn get(&self, key: &str) -> Option<u64> {
        self.inner.get(key.as_bytes())
    }

    /// Deletes a string key from the map. Returns true if removed.
    pub fn delete(&mut self, key: &str) -> bool {
        self.inner.remove(key.as_bytes()).is_some()
    }

    /// Checks if a string key exists in the map.
    pub fn has(&self, key: &str) -> bool {
        self.inner.get(key.as_bytes()).is_some()
    }

    /// Returns the count of entries in the map.
    pub fn count(&self) -> usize {
        self.inner.len() as usize
    }

    /// Clears all entries from the map.
    pub fn clear(&mut self) {
        self.inner.clear();
    }

    /// Returns the exact memory used by the string map in bytes.
    #[php(name = "memUsed")]
    pub fn mem_used(&self) -> usize {
        self.inner.mem_used()
    }
}

/// Native Expanse Bytes Map (`Expanse\ExpanseBytesMap`)
#[php_class]
#[php(name = "Expanse\\ExpanseBytesMap")]
#[derive(Default)]
pub struct PhpExpanseBytesMap {
    inner: ExpanseBytesMap,
}

#[php_impl]
impl PhpExpanseBytesMap {
    /// Creates a new empty ExpanseBytesMap.
    pub fn __construct() -> Self {
        Self::default()
    }

    /// Sets binary byte string key -> integer value.
    pub fn set(&mut self, key: String, value: u64) {
        self.inner.insert(key.as_bytes(), value);
    }

    /// Gets the integer value associated with binary key, or null if absent.
    pub fn get(&self, key: String) -> Option<u64> {
        self.inner.get(key.as_bytes())
    }

    /// Deletes a binary key from the map. Returns true if removed.
    pub fn delete(&mut self, key: String) -> bool {
        self.inner.remove(key.as_bytes()).is_some()
    }

    /// Checks if a binary key exists in the map.
    pub fn has(&self, key: String) -> bool {
        self.inner.contains_key(key.as_bytes())
    }

    /// Returns the count of entries in the map.
    pub fn count(&self) -> usize {
        self.inner.len() as usize
    }

    /// Clears all entries from the map.
    pub fn clear(&mut self) {
        self.inner.clear();
    }

    /// Returns the exact memory used by the bytes map in bytes.
    #[php(name = "memUsed")]
    pub fn mem_used(&self) -> usize {
        self.inner.mem_used()
    }
}

/// Native Expanse Blob Map (`Expanse\ExpanseBlobMap`)
#[php_class]
#[php(name = "Expanse\\ExpanseBlobMap")]
#[derive(Default)]
pub struct PhpExpanseBlobMap {
    inner: ExpanseBlobMap,
}

#[php_impl]
impl PhpExpanseBlobMap {
    /// Creates a new empty ExpanseBlobMap.
    pub fn __construct() -> Self {
        Self::default()
    }

    /// Sets key -> payload blob with optional 32-bit hot metadata.
    pub fn set(&mut self, key: u64, payload: String, hot_meta: Option<u32>) -> Result<(), String> {
        self.inner
            .insert(key, payload.as_bytes(), hot_meta.unwrap_or(0))
            .map_err(|e| format!("{e:?}"))
    }

    /// Gets the payload blob associated with key, or null if absent.
    pub fn get(&self, key: u64) -> Option<String> {
        self.inner
            .get(key)
            .map(|(view, _)| String::from_utf8_lossy(view.as_bytes()).into_owned())
    }

    /// Gets the 32-bit hot metadata associated with key, or null if absent.
    #[php(name = "getMeta")]
    pub fn get_meta(&self, key: u64) -> Option<u32> {
        self.inner.get(key).map(|(_, meta)| meta)
    }

    /// Deletes a key from the blob map. Returns true if removed.
    pub fn delete(&mut self, key: u64) -> bool {
        self.inner.remove(key)
    }

    /// Checks if a key exists in the blob map.
    pub fn has(&self, key: u64) -> bool {
        self.inner.get(key).is_some()
    }

    /// Returns the total count of blobs in the map.
    pub fn count(&self) -> usize {
        self.inner.len() as usize
    }

    /// Clears all entries from the blob map.
    pub fn clear(&mut self) {
        self.inner.clear();
    }

    /// Returns the exact memory used by the blob map and arena in bytes.
    #[php(name = "memUsed")]
    pub fn mem_used(&self) -> usize {
        self.inner.mem_used()
    }
}

/// Native Concurrent Expanse Set (`Expanse\SyncSet`)
#[php_class]
#[php(name = "Expanse\\SyncSet")]
#[derive(Default)]
pub struct PhpSyncSet {
    inner: SyncExpanseSet,
}

#[php_impl]
impl PhpSyncSet {
    /// Creates a new thread-safe SyncSet.
    pub fn __construct() -> Self {
        Self::default()
    }

    /// Adds a key concurrently. Returns true if newly inserted.
    pub fn add(&self, key: u64) -> bool {
        self.inner.insert(key)
    }

    /// Removes a key concurrently. Returns true if removed.
    pub fn remove(&self, key: u64) -> bool {
        self.inner.remove(key)
    }

    /// Checks if a key exists lock-free.
    pub fn contains(&self, key: u64) -> bool {
        self.inner.contains(key)
    }
}

/// Native Concurrent Expanse Map (`Expanse\SyncMap`)
#[php_class]
#[php(name = "Expanse\\SyncMap")]
#[derive(Default)]
pub struct PhpSyncMap {
    inner: SyncExpanseMap,
}

#[php_impl]
impl PhpSyncMap {
    /// Creates a new thread-safe SyncMap.
    pub fn __construct() -> Self {
        Self::default()
    }

    /// Sets key -> value concurrently.
    pub fn set(&self, key: u64, value: u64) {
        self.inner.insert(key, value);
    }

    /// Gets value lock-free.
    pub fn get(&self, key: u64) -> Option<u64> {
        self.inner.get(key)
    }

    /// Deletes a key concurrently. Returns true if removed.
    pub fn delete(&self, key: u64) -> bool {
        self.inner.remove(key).is_some()
    }
}

/// Legacy Judy compatibility class (`Judy`)
#[php_class]
#[php(name = "Expanse\\ExpanseJudy")]
pub struct PhpJudyCompat {
    judy_type: i64,
    set_inner: Option<ExpanseSet>,
    map_inner: Option<ExpanseMap>,
    strmap_inner: Option<ExpanseStrMap>,
}

#[php_impl]
impl PhpJudyCompat {
    /// Bitset type (Judy1)
    pub const BITSET: i64 = 1;
    /// Integer to Integer Map (JudyL)
    pub const INT_TO_INT: i64 = 2;
    /// Integer to Mixed Map (JudyL word value)
    pub const INT_TO_MIXED: i64 = 3;
    /// String to Integer Map (JudySL)
    pub const STRING_TO_INT: i64 = 4;
    /// String to Mixed Map (JudySL word value)
    pub const STRING_TO_MIXED: i64 = 5;

    /// Initializes a new Judy array of the given type.
    pub fn __construct(judy_type: i64) -> Result<Self, String> {
        match judy_type {
            Self::BITSET => Ok(Self {
                judy_type,
                set_inner: Some(ExpanseSet::new()),
                map_inner: None,
                strmap_inner: None,
            }),
            Self::INT_TO_INT | Self::INT_TO_MIXED => Ok(Self {
                judy_type,
                set_inner: None,
                map_inner: Some(ExpanseMap::new()),
                strmap_inner: None,
            }),
            Self::STRING_TO_INT | Self::STRING_TO_MIXED => Ok(Self {
                judy_type,
                set_inner: None,
                map_inner: None,
                strmap_inner: Some(ExpanseStrMap::new()),
            }),
            other => Err(format!("Unsupported Judy type: {other}")),
        }
    }

    /// Returns the Judy array type constant.
    #[php(name = "getType")]
    pub fn get_type(&self) -> i64 {
        self.judy_type
    }

    /// Returns the population / count of entries.
    pub fn count(&self) -> usize {
        if let Some(set) = &self.set_inner {
            set.len() as usize
        } else if let Some(map) = &self.map_inner {
            map.len() as usize
        } else if let Some(strmap) = &self.strmap_inner {
            strmap.len() as usize
        } else {
            0
        }
    }

    /// Returns the memory used in bytes.
    #[php(name = "memoryUsage")]
    pub fn memory_usage(&self) -> usize {
        if let Some(set) = &self.set_inner {
            set.mem_used()
        } else if let Some(map) = &self.map_inner {
            map.mem_used()
        } else if let Some(strmap) = &self.strmap_inner {
            strmap.mem_used()
        } else {
            0
        }
    }

    /// Frees the entire Judy array and returns the number of bytes released.
    pub fn free(&mut self) -> usize {
        let mem = self.memory_usage();
        if let Some(set) = &mut self.set_inner {
            set.clear();
        } else if let Some(map) = &mut self.map_inner {
            map.clear();
        } else if let Some(strmap) = &mut self.strmap_inner {
            strmap.clear();
        }
        mem
    }
}

/// Zend Engine module definition function.
#[php_module]
pub fn get_module(module: ModuleBuilder) -> ModuleBuilder {
    module
        .class::<PhpExpanseSet>()
        .class::<PhpExpanseMap>()
        .class::<PhpExpanseStrMap>()
        .class::<PhpExpanseBytesMap>()
        .class::<PhpExpanseBlobMap>()
        .class::<PhpSyncSet>()
        .class::<PhpSyncMap>()
        .class::<PhpJudyCompat>()
}
