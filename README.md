# dynamic-lru-cache

[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](
https://github.com/Cognoscan/fog-crypto)
[![Cargo](https://img.shields.io/crates/v/dynamic-lru-cache.svg)](
https://crates.io/crates/dynamic-lru-cache)
[![Documentation](https://docs.rs/dynamic-lru-cache/badge.svg)](
https://docs.rs/dynamic-lru-cache)

A simple LRU cache for Rust that only caches items it has seen at least once 
before. The size of its internal memory is adjustable.

## Why?

I didn't want to use a fixed cache size when I expect that most data will not be 
fetched twice, and that most of the time the number of items benefit from 
caching will be small. Good use cases: parsing large data structures that 
frequently cross-reference the same data chunk, reading a set of 
dictionary-compressed files where there are several different but shared 
dictionary, reading many files that all refer to shared parser profiles (eg. 
color profiles in images), etc.

Sure, a fixed size cache that stores "seen once" items would also work, but the 
memory usage would be higher than really necessary. Hence, this crate.
