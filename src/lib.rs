//! A simple LRU cache for Rust that only caches items it has seen at least once 
//! before. The size of its internal memory is adjustable.
//! 
//! ## Why?
//! 
//! I didn't want to use a fixed cache size when I expect that most data will not be 
//! fetched twice, and that most of the time the number of items benefit from 
//! caching will be small. Good use cases: parsing large data structures that 
//! frequently cross-reference the same data chunk, reading a set of 
//! dictionary-compressed files where there are several different but shared 
//! dictionary, reading many files that all refer to shared parser profiles (eg. 
//! color profiles in images), etc.
//! 
//! Sure, a fixed size cache that stores "seen once" items would also work, but the 
//! memory usage would be higher than really necessary. Hence, this crate.

use std::collections::hash_map::RandomState;
use std::collections::{HashMap, VecDeque};
use std::hash::Hash;
use std::sync::Arc;
use parking_lot::Mutex;
use std::fmt;

/// A cache that will only hold onto items that have been reqeuested more than once in recent 
/// memory. Single-use items are not held at all. Once an item is requested twice, it is cached 
/// until all memory of seeing it requested has expired. The length of the memory is adjustable, 
/// and must be set at initialization.
pub struct DynamicCacheLocal <K, V, S = RandomState > {
    map: HashMap<K,(u32, Option<Arc<V>>),S>,
    list: VecDeque<(K,u32)>,
    mem_len: usize,
    size: usize,
    hits: u64,
    misses: u64,
}

impl <K: Clone + Eq + Hash, V, S> DynamicCacheLocal<K, V, S> {

    /// Create and initialize a new cache, using the given hash builder to hash keys. The same 
    /// warnings given for [`HashMap::with_hasher`] apply here.
    pub fn with_hasher(mem_len: usize, hash_builder: S) -> DynamicCacheLocal<K, V, S> {
        // Just make it work if an invalid value is thrown in
        let mem_len = mem_len.clamp(2, u32::MAX as usize);

        Self {
            map: HashMap::with_hasher(hash_builder),
            list: VecDeque::with_capacity(mem_len),
            mem_len,
            size: 0,
            hits: 0,
            misses: 0,
        }
    }
}

impl <K: Clone + Eq + Hash, V> DynamicCacheLocal<K, V> {

    /// Create and initialize a new cache.
    pub fn new(mem_len: usize) -> Self {
        // Just make it work if an invalid value is thrown in
        let mem_len = mem_len.clamp(2, u32::MAX as usize);

        Self {
            map: HashMap::new(),
            list: VecDeque::with_capacity(mem_len),
            mem_len,
            size: 0,
            hits: 0,
            misses: 0,
        }
    }

    fn get(&mut self, key: &K) -> Option<Arc<V>> {
        let (counter, ret) = match self.map.get_mut(key) {
            Some((counter, Some(v))) => {
                *counter += 1;
                (*counter, Some(v.clone()))
            },
            Some((counter, None)) => {
                *counter += 1;
                (*counter, None)
            },
            None => {
                self.map.insert(key.clone(), (0, None));
                (0, None)
            }
        };

        if self.list.len() == self.mem_len {
            let (key, last_count) = self.list.pop_back()
                .expect("Cache memory queue should be non-empty at this point");
            let (counter, val) = self.map.get(&key)
                .expect("Cache hashmap should contain the key from the memory queue");
            if *counter == last_count
            {
                if val.is_some() { self.size -= 1; }
                self.map.remove(&key);
            }
        }
        self.list.push_front((key.clone(), counter));

        if ret.is_some() {
            self.hits += 1;
        }
        else {
            self.misses += 1;
        }

        ret
    }

    fn insert(&mut self, key: &K, v: V) -> Arc<V> {
        let (counter, val) = self.map.get_mut(key).expect("Cache hashmap should have this key");
        if *counter == 0 { Arc::new(v) }
        else if let Some(val) = val {
            val.clone()
        }
        else {
            let v = Arc::new(v);
            *val = Some(v.clone());
            self.size += 1;
            v
        }
    }

    /// Fetch an item via the cache, potentially filling in the cache on a miss via the function 
    /// `f`.
    pub fn get_or_insert<F: FnOnce() -> V>(&mut self, key: &K, f: F) -> Arc<V> {
        self.get(key).unwrap_or_else(|| self.insert(key, f()))
    }

    /// Get the number of items currently stored in the cache.
    pub fn size(&self) -> usize {
        self.size
    }

    /// Get the length of the cache's recent request memory.
    pub fn mem_len(&self) -> usize {
        self.mem_len
    }

    /// Change the length of the cache's recent request memory. Some contents of the cache may be 
    /// removed immediately if the new memory length is shorter than the old memory length.
    pub fn set_mem_len(&mut self, new_len: usize) {
        // Just make it work if an invalid value is thrown in
        let new_len = new_len.clamp(2, u32::MAX as usize);
        // Remove any excess memory
        while self.list.len() > new_len {
            let (key, last_count) = self.list.pop_back()
                .expect("Cache memory queue should be non-empty at this point");
            let (counter, val) = self.map.get(&key)
                .expect("Cache hashmap should contain the key from the memory queue");
            if *counter == last_count
            {
                if val.is_some() { self.size -= 1; }
                self.map.remove(&key);
            }
        }
        self.mem_len = new_len;
    }

    /// Clear out all stored values and all memory in the cache.
    pub fn clear_cache(&mut self) {
        self.size = 0;
        self.map.clear();
        self.list.clear();
    }

    /// Get the number of hits this cache has seen.
    pub fn hits(&self) -> u64 {
        self.hits
    }

    /// Get the number of misses this cache has seen.
    pub fn misses(&self) -> u64 {
        self.misses
    }

    /// Reset the cache hit/miss metrics.
    pub fn reset_metrics(&mut self) {
        self.hits = 0;
        self.misses = 0;
    }

}

impl<K: fmt::Debug, V: fmt::Debug, S> fmt::Debug for DynamicCacheLocal<K, V, S>
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {

        f.debug_struct("DynamicCacheLocal")
            .field("map", &format!("{} entries", self.map.len()))
            .field("list", &format!("{} long", self.list.len()))
            .field("mem_len", &self.mem_len)
            .field("size", &self.size)
            .finish()
    }
}

/// A cache that will only hold onto items that have been reqeuested more than once in recent 
/// memory. Single-use items are not held at all. Once an item is requested twice, it is cached 
/// until all memory of seeing it requested has expired. The length of the memory is adjustable, 
/// and must be set at initialization.
///
/// This version of the cache can be shared across threads without issue, as it is an instance of 
/// [`DynamicCacheLocal`] held by an `Arc<Mutex<T>>`.
#[derive(Clone, Debug)]
pub struct DynamicCache<K, V, S = RandomState> {
    cache: Arc<Mutex<DynamicCacheLocal<K, V, S>>>,
}

impl <K: Clone + Eq + Hash, V, S> DynamicCache<K, V, S> {

    /// Create and initialize a new cache, using the given hash builder to hash keys. The same 
    /// warnings given for [`HashMap::with_hasher`] apply here.
    pub fn with_hasher(mem_len: usize, hash_builder: S) -> DynamicCache<K, V, S> {
        Self { cache: Arc::new(Mutex::new(DynamicCacheLocal::with_hasher(mem_len, hash_builder))) }
    }
}

impl <K: Clone + Eq + Hash, V> DynamicCache<K, V> {

    /// Create an initialize a new cache.
    pub fn new(mem_len: usize) -> Self {
        Self { cache: Arc::new(Mutex::new(DynamicCacheLocal::new(mem_len))) }
    }

    fn get(&self, key: &K) -> Option<Arc<V>> {
        self.cache.lock().get(key)
    }

    fn insert(&self, key: &K, value: V) -> Arc<V> {
        self.cache.lock().insert(key, value)
    }

    /// Fetch an item via the cache, potentially filling in the cache on a miss via the function 
    /// `f`. The cache is unlocked while calling the function, so `f` may be called more than once 
    /// with the same parameters if there are several threads using the cache.
    pub fn get_or_insert<F: FnOnce() -> V>(&self, key: &K, f: F) -> Arc<V> {
        self.get(key).unwrap_or_else(|| self.insert(key, f()))
    }

    /// Get the number of items currently stored in the cache.
    pub fn size(&self) -> usize {
        self.cache.lock().size()
    }

    /// Get the length of the cache's recent request memory.
    pub fn mem_len(&self) -> usize {
        self.cache.lock().mem_len()
    }

    /// Change the length of the cache's recent request memory. Some contents of the cache may be 
    /// removed immediately if the new memory length is shorter than the old memory length.
    pub fn set_mem_len(&self, new_len: usize) {
        self.cache.lock().set_mem_len(new_len)
    }

    /// Clear out all stored values and all memory in the cache.
    pub fn clear_cache(&self) {
        self.cache.lock().clear_cache()
    }

    /// Get the cache metrics as a pair `(hits, misses)`.
    pub fn hits_misses(&self) -> (u64, u64) {
        let cache = self.cache.lock();
        (cache.hits(), cache.misses())
    }

    /// Reset the cache hit/miss metrics.
    pub fn reset_metrics(&self) {
        self.cache.lock().reset_metrics()
    }

}


#[cfg(test)]
mod test {
    use super::*;
    use rand::prelude::*;

    #[test]
    fn do_it() {
        let sample_size = 1<<12;
        let cache = DynamicCache::new(128);

        let mut rng = thread_rng();

        let seq: Vec<u16> = vec![0,0,0,0,1,1,0,1,0,1,2,0,1,2,0,1,2,0,1,2,0,1,2,0,1,2];

        for key in seq {
            println!("Write {}", key);
            let val = format!("{}", key);
            let cache_val = if let Some(v) = cache.get(&key) {
                println!("Hit");
                v
            } else {
                println!("Miss");
                cache.insert(&key, val.clone())
            };
            assert_eq!(val.as_str(), cache_val.as_str());
        }
        println!("Cache size: {}", cache.size());

        for i in (3..=9).rev() {
            let mut misses = 0;
            for _ in 0..sample_size {
                let key: u16 = rng.gen_range(0, 1<<i);
                let val = format!("{}", key);
                let cache_val = if let Some(v) = cache.get(&key) { v } else {
                    misses += 1;
                    cache.insert(&key, val.clone())
                };
                assert_eq!(val.as_str(), cache_val.as_str());
            }
            let hit_rate = 100.0 * f64::from(sample_size-misses) / f64::from(sample_size);
            println!("With range of (0..{:3}), Cache size: {:3}, hit rate = {:4.1}%", (1<<i), cache.size(), hit_rate);
        }

        let mut misses = 0;
        for _ in 0..sample_size {
            let key: u16 = rng.gen();
            let val = format!("{}", key);
            let cache_val = if let Some(v) = cache.get(&key) { v } else {
                misses += 1;
                cache.insert(&key, val.clone())
            };
            assert_eq!(val.as_str(), cache_val.as_str());
        }
        let hit_rate = 100.0 * f64::from(sample_size-misses) / f64::from(sample_size);
        println!("With range of full u16, Cache size: {:3}, hit rate = {:4.1}%", cache.size(), hit_rate);

        let weights: Vec<u32> = vec![16,8,4,2,1];
        let dist = rand::distributions::WeightedIndex::new(&weights).unwrap();
        let mut misses = 0;
        for _ in 0..sample_size {
            let is_main = rng.gen_bool(0.5);
            let key: u16 = if is_main {
                dist.sample(&mut rng) as u16
            }
            else {
                rng.gen()
            };
            let val = format!("{}", key);
            let cache_val = if let Some(v) = cache.get(&key) { v } else {
                if is_main { misses += 1; }
                cache.insert(&key, val.clone())
            };
            assert_eq!(val.as_str(), cache_val.as_str());
        }
        let hit_rate = 100.0 * f64::from(sample_size-misses) / f64::from(sample_size);
        println!("Random u16 with log2 frequent requests, Cache size: {:3}, hit rate for main data = {:4.1}%", cache.size(), hit_rate);

        panic!();
    }
}
