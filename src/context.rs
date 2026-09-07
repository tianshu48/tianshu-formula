//! Variable context.

use rustc_hash::FxHashMap;

pub trait Context {
    fn get(&self, name: &str) -> Option<f64>;
}

impl Context for FxHashMap<String, f64> {
    fn get(&self, name: &str) -> Option<f64> {
        FxHashMap::get(self, name).copied()
    }
}

impl Context for std::collections::HashMap<String, f64> {
    fn get(&self, name: &str) -> Option<f64> {
        std::collections::HashMap::get(self, name).copied()
    }
}

#[derive(Debug, Clone, Default)]
pub struct MapContext {
    map: FxHashMap<String, f64>,
}

impl MapContext {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, name: impl Into<String>, value: f64) -> &mut Self {
        self.map.insert(name.into(), value);
        self
    }

    pub fn from_pairs<I, S>(pairs: I) -> Self
    where
        I: IntoIterator<Item = (S, f64)>,
        S: Into<String>,
    {
        let mut ctx = Self::new();
        for (k, v) in pairs {
            ctx.insert(k, v);
        }
        ctx
    }
}

impl Context for MapContext {
    fn get(&self, name: &str) -> Option<f64> {
        self.map.get(name).copied()
    }
}

impl Context for &MapContext {
    fn get(&self, name: &str) -> Option<f64> {
        (*self).get(name)
    }
}
