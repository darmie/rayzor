//! Symbol resolution cache implementation for performance optimization
//!
//! This module provides a caching layer for symbol resolution to avoid
//! repeated lookups through the scope hierarchy.

use std::cell::RefCell;
use std::collections::HashMap;
use crate::tast::{InternedString, ScopeId, SymbolId};

/// Cache key for symbol resolution
#[derive(Hash, Eq, PartialEq, Clone, Debug)]
pub struct SymbolCacheKey {
    pub scope: ScopeId,
    pub name: InternedString,
}

/// Statistics for cache performance monitoring
#[derive(Default, Debug)]
pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub invalidations: u64,
}

impl CacheStats {
    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            (self.hits as f64 / total as f64) * 100.0
        }
    }
}

/// Symbol resolution cache with invalidation support
pub struct SymbolResolutionCache {
    /// The actual cache storage
    cache: RefCell<HashMap<SymbolCacheKey, Option<SymbolId>>>,
    
    /// Cache statistics
    stats: RefCell<CacheStats>,
    
    /// Maximum cache size before eviction
    max_size: usize,
}

impl SymbolResolutionCache {
    /// Create a new symbol resolution cache
    pub fn new(max_size: usize) -> Self {
        Self {
            cache: RefCell::new(HashMap::with_capacity(max_size / 2)),
            stats: RefCell::new(CacheStats::default()),
            max_size,
        }
    }
    
    /// Look up a symbol in the cache
    pub fn get(&self, scope: ScopeId, name: InternedString) -> Option<Option<SymbolId>> {
        let key = SymbolCacheKey { scope, name };
        let mut stats = self.stats.borrow_mut();
        
        if let Some(&result) = self.cache.borrow().get(&key) {
            stats.hits += 1;
            Some(result)
        } else {
            stats.misses += 1;
            None
        }
    }
    
    /// Insert a symbol resolution result into the cache
    pub fn insert(&self, scope: ScopeId, name: InternedString, symbol: Option<SymbolId>) {
        let mut cache = self.cache.borrow_mut();
        
        // Check if we need to evict entries
        if cache.len() >= self.max_size {
            self.evict_oldest(&mut cache);
        }
        
        let key = SymbolCacheKey { scope, name };
        cache.insert(key, symbol);
    }
    
    /// Invalidate all cached entries for a specific scope
    pub fn invalidate_scope(&self, scope: ScopeId) {
        let mut cache = self.cache.borrow_mut();
        let mut stats = self.stats.borrow_mut();
        
        // Remove all entries for this scope
        cache.retain(|key, _| {
            if key.scope == scope {
                stats.invalidations += 1;
                false
            } else {
                true
            }
        });
    }
    
    /// Invalidate all cached entries for a specific symbol name
    pub fn invalidate_name(&self, name: InternedString) {
        let mut cache = self.cache.borrow_mut();
        let mut stats = self.stats.borrow_mut();
        
        // Remove all entries for this name
        cache.retain(|key, _| {
            if key.name == name {
                stats.invalidations += 1;
                false
            } else {
                true
            }
        });
    }
    
    /// Clear the entire cache
    pub fn clear(&self) {
        let mut cache = self.cache.borrow_mut();
        let mut stats = self.stats.borrow_mut();
        stats.invalidations += cache.len() as u64;
        cache.clear();
    }
    
    /// Get cache statistics
    pub fn stats(&self) -> CacheStats {
        self.stats.borrow().clone()
    }
    
    /// Evict oldest entries when cache is full (simple FIFO for now)
    fn evict_oldest(&self, cache: &mut HashMap<SymbolCacheKey, Option<SymbolId>>) {
        // Remove 10% of entries
        let to_remove = self.max_size / 10;
        let keys: Vec<_> = cache.keys().take(to_remove).cloned().collect();
        
        for key in keys {
            cache.remove(&key);
        }
        
        self.stats.borrow_mut().invalidations += to_remove as u64;
    }
}

/// Bloom filter for quick negative lookups
pub struct SymbolBloomFilter {
    bits: Vec<u64>,
    num_hashes: usize,
}

impl SymbolBloomFilter {
    /// Create a new bloom filter with the specified size
    pub fn new(expected_items: usize) -> Self {
        // Calculate optimal size (10 bits per item for ~1% false positive rate)
        let num_bits = expected_items * 10;
        let num_words = (num_bits + 63) / 64;
        
        Self {
            bits: vec![0; num_words],
            num_hashes: 7, // Optimal for 10 bits per item
        }
    }
    
    /// Add a symbol name to the bloom filter
    pub fn insert(&mut self, name: InternedString) {
        let hash = name.as_u64();
        
        for i in 0..self.num_hashes {
            let bit_pos = self.hash_to_bit(hash, i);
            let word_idx = bit_pos / 64;
            let bit_idx = bit_pos % 64;
            
            if word_idx < self.bits.len() {
                self.bits[word_idx] |= 1u64 << bit_idx;
            }
        }
    }
    
    /// Check if a symbol name might be in the set
    pub fn might_contain(&self, name: InternedString) -> bool {
        let hash = name.as_u64();
        
        for i in 0..self.num_hashes {
            let bit_pos = self.hash_to_bit(hash, i);
            let word_idx = bit_pos / 64;
            let bit_idx = bit_pos % 64;
            
            if word_idx >= self.bits.len() {
                return false;
            }
            
            if (self.bits[word_idx] & (1u64 << bit_idx)) == 0 {
                return false;
            }
        }
        
        true
    }
    
    /// Clear the bloom filter
    pub fn clear(&mut self) {
        self.bits.fill(0);
    }
    
    /// Generate a bit position from a hash and index
    fn hash_to_bit(&self, hash: u64, index: usize) -> usize {
        // Use different parts of the hash for each index
        let shifted = hash.wrapping_add(index as u64).wrapping_mul(0x517cc1b727220a95);
        (shifted as usize) % (self.bits.len() * 64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tast::{StringInterner, new_scope_id, new_symbol_id};
    
    #[test]
    fn test_symbol_cache_basic() {
        let cache = SymbolResolutionCache::new(100);
        let mut interner = StringInterner::new();
        
        let scope1 = new_scope_id();
        let name1 = interner.intern("test_symbol");
        let symbol1 = new_symbol_id();
        
        // Cache miss
        assert_eq!(cache.get(scope1, name1), None);
        assert_eq!(cache.stats().misses, 1);
        
        // Insert
        cache.insert(scope1, name1, Some(symbol1));
        
        // Cache hit
        assert_eq!(cache.get(scope1, name1), Some(Some(symbol1)));
        assert_eq!(cache.stats().hits, 1);
        
        // Different scope = cache miss
        let scope2 = new_scope_id();
        assert_eq!(cache.get(scope2, name1), None);
        assert_eq!(cache.stats().misses, 2);
    }
    
    #[test]
    fn test_bloom_filter() {
        let mut bloom = SymbolBloomFilter::new(1000);
        let mut interner = StringInterner::new();
        
        let name1 = interner.intern("test1");
        let name2 = interner.intern("test2");
        let name3 = interner.intern("test3");
        
        // Insert some names
        bloom.insert(name1);
        bloom.insert(name2);
        
        // Check containment
        assert!(bloom.might_contain(name1));
        assert!(bloom.might_contain(name2));
        
        // Name3 not inserted - might still return true (false positive)
        // but should return false most of the time
        let might_contain_name3 = bloom.might_contain(name3);
        
        // Clear and recheck
        bloom.clear();
        assert!(!bloom.might_contain(name1));
        assert!(!bloom.might_contain(name2));
    }
}