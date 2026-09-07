//! Compile-once formula cache (folded AST), keyed by source text.

use std::sync::Arc;

use rustc_hash::FxHashMap;

use crate::builtins::Registry;
use crate::error::Error;
use crate::eval::Formula;

/// Default soft cap; oldest insertion order is dropped when exceeded.
pub const DEFAULT_FORMULA_CACHE_CAPACITY: usize = 512;

/// Deep module: get-or-parse folded [`Formula`] values without callers caring about hits.
#[derive(Debug, Clone, Default)]
pub struct FormulaCache {
    map: FxHashMap<String, Arc<Formula>>,
    order: Vec<String>,
    capacity: usize,
}

impl FormulaCache {
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_FORMULA_CACHE_CAPACITY)
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            map: FxHashMap::default(),
            order: Vec::new(),
            capacity: capacity.max(1),
        }
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    pub fn clear(&mut self) {
        self.map.clear();
        self.order.clear();
    }

    /// Parse+fold `source` on miss; return shared compiled formula. Failed parses are not cached.
    pub fn get_or_parse(&mut self, source: &str) -> Result<Arc<Formula>, Error> {
        self.get_or_parse_with(source, None)
    }

    pub fn get_or_parse_with(
        &mut self,
        source: &str,
        registry: Option<&Registry>,
    ) -> Result<Arc<Formula>, Error> {
        if let Some(hit) = self.map.get(source) {
            return Ok(Arc::clone(hit));
        }
        let formula = Formula::parse_with(source, registry)?;
        let key = source.to_string();
        let arc = Arc::new(formula);
        self.insert(key, Arc::clone(&arc));
        Ok(arc)
    }

    // ponytail: eviction mutates the map, so we cannot hold a vacant Entry.
    #[allow(clippy::map_entry)]
    fn insert(&mut self, key: String, value: Arc<Formula>) {
        if self.map.contains_key(&key) {
            self.map.insert(key, value);
            return;
        }
        while self.map.len() >= self.capacity && !self.order.is_empty() {
            let oldest = self.order.remove(0);
            self.map.remove(&oldest);
        }
        self.order.push(key.clone());
        self.map.insert(key, value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::MapContext;
    use crate::error::ErrorKind;

    #[test]
    fn hit_returns_same_arc_and_eval() {
        let mut cache = FormulaCache::new();
        let a = cache.get_or_parse("1 + 2 * 3").unwrap();
        let b = cache.get_or_parse("1 + 2 * 3").unwrap();
        assert!(Arc::ptr_eq(&a, &b));
        assert_eq!(cache.len(), 1);
        assert_eq!(a.eval(&MapContext::new()).unwrap(), 7.0);
        assert_eq!(b.eval(&MapContext::new()).unwrap(), 7.0);
    }

    #[test]
    fn parse_error_does_not_pollute_cache() {
        let mut cache = FormulaCache::new();
        let err = cache.get_or_parse("1 +").unwrap_err();
        assert_eq!(err.kind, ErrorKind::Parse);
        assert!(cache.is_empty());
        let ok = cache.get_or_parse("1 + 2").unwrap();
        assert_eq!(ok.eval(&MapContext::new()).unwrap(), 3.0);
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn capacity_evicts_oldest() {
        let mut cache = FormulaCache::with_capacity(2);
        let _ = cache.get_or_parse("1").unwrap();
        let _ = cache.get_or_parse("2").unwrap();
        assert_eq!(cache.len(), 2);
        let _ = cache.get_or_parse("3").unwrap();
        assert_eq!(cache.len(), 2);
        assert!(!cache.map.contains_key("1"));
        assert!(cache.map.contains_key("2"));
        assert!(cache.map.contains_key("3"));
    }
}
