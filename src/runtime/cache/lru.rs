use std::{
    collections::{HashMap, VecDeque},
    hash::Hash,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CacheMutationReport<K> {
    pub evicted: Vec<K>,
    pub replaced: bool,
    pub total_weight: usize,
    pub utilization: usize,
}

impl<K> Default for CacheMutationReport<K> {
    fn default() -> Self {
        Self {
            evicted: Vec::new(),
            replaced: false,
            total_weight: 0,
            utilization: 0,
        }
    }
}

#[derive(Clone, Debug)]
pub struct BoundedLruCache<K, V> {
    capacity: usize,
    map: HashMap<K, V>,
    order: VecDeque<K>,
    weights: HashMap<K, usize>,
    total_weight: usize,
}

impl<K, V> BoundedLruCache<K, V>
where
    K: Clone + Eq + Hash,
{
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            map: HashMap::new(),
            order: VecDeque::new(),
            weights: HashMap::new(),
            total_weight: 0,
        }
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    pub fn get_cloned(&mut self, key: &K) -> Option<V>
    where
        V: Clone,
    {
        let value = self.map.get(key).cloned()?;
        self.touch(key);
        Some(value)
    }

    pub fn insert(&mut self, key: K, value: V) -> CacheMutationReport<K> {
        self.insert_with_weight(key, value, 1)
    }

    pub fn insert_with_weight(&mut self, key: K, value: V, weight: usize) -> CacheMutationReport<K> {
        if self.capacity == 0 {
            return CacheMutationReport::default();
        }

        let weight = weight.max(1);
        let key_for_order = key.clone();

        // If replacing, subtract old weight first
        let replaced = if let Some(old_weight) = self.weights.get(&key).copied() {
            self.total_weight -= old_weight;
            true
        } else {
            false
        };

        self.weights.insert(key.clone(), weight);
        self.total_weight += weight;
        self.map.insert(key, value);
        self.touch_owned(key_for_order);

        let evicted = if !replaced {
            self.evict_if_needed()
        } else {
            Vec::new()
        };

        CacheMutationReport {
            total_weight: self.total_weight,
            utilization: if self.capacity > 0 {
                self.map.len() * 100 / self.capacity
            } else {
                0
            },
            evicted,
            replaced,
        }
    }

    pub fn total_weight(&self) -> usize {
        self.total_weight
    }

    fn evict_if_needed(&mut self) -> Vec<K> {
        let mut evicted = Vec::new();
        while self.map.len() > self.capacity {
            // Find entry with lowest recency_rank / weight ratio
            // (heavy + stale entries get evicted first)
            let victim = self
                .order
                .iter()
                .enumerate()
                .min_by(|(rank_a, key_a), (rank_b, key_b)| {
                    let weight_a = *self.weights.get(key_a).unwrap_or(&1);
                    let weight_b = *self.weights.get(key_b).unwrap_or(&1);
                    let score_a = (*rank_a as f64) / (weight_a as f64);
                    let score_b = (*rank_b as f64) / (weight_b as f64);
                    score_a.partial_cmp(&score_b).unwrap_or(std::cmp::Ordering::Equal)
                })
                .map(|(_, key)| key.clone());

            if let Some(key) = victim {
                if let Some(index) = self.order.iter().position(|k| k == &key) {
                    self.order.remove(index);
                }
                if let Some(w) = self.weights.remove(&key) {
                    self.total_weight -= w;
                }
                self.map.remove(&key);
                evicted.push(key);
            } else {
                break;
            }
        }
        evicted
    }

    fn touch(&mut self, key: &K) {
        if let Some(index) = self.order.iter().position(|existing| existing == key) {
            self.order.remove(index);
        }
        self.order.push_back(key.clone());
    }

    fn touch_owned(&mut self, key: K) {
        if let Some(index) = self.order.iter().position(|existing| existing == &key) {
            self.order.remove(index);
        }
        self.order.push_back(key);
    }
}

#[cfg(test)]
mod tests {
    use super::BoundedLruCache;

    #[test]
    fn evicts_oldest_entry_when_capacity_is_exceeded() {
        let mut cache = BoundedLruCache::new(2);
        cache.insert("a", 1);
        cache.insert("b", 2);
        cache.insert("c", 3);

        assert_eq!(cache.len(), 2);
        assert_eq!(cache.get_cloned(&"a"), None);
        assert_eq!(cache.get_cloned(&"b"), Some(2));
        assert_eq!(cache.get_cloned(&"c"), Some(3));
    }

    #[test]
    fn get_refreshes_recency() {
        let mut cache = BoundedLruCache::new(2);
        cache.insert("a", 1);
        cache.insert("b", 2);

        assert_eq!(cache.get_cloned(&"a"), Some(1));
        cache.insert("c", 3);

        assert_eq!(cache.get_cloned(&"a"), Some(1));
        assert_eq!(cache.get_cloned(&"b"), None);
        assert_eq!(cache.get_cloned(&"c"), Some(3));
    }

    #[test]
    fn zero_capacity_drops_all_inserts() {
        let mut cache = BoundedLruCache::new(0);
        cache.insert("a", 1);
        cache.insert("b", 2);

        assert!(cache.is_empty());
        assert_eq!(cache.get_cloned(&"a"), None);
    }

    #[test]
    fn heavier_entry_is_evicted_before_lighter_peer_when_capacity_is_exceeded() {
        let mut cache = BoundedLruCache::new(2);
        cache.insert_with_weight("heavy", 1, 32);
        cache.insert_with_weight("light", 2, 1);
        let report = cache.insert_with_weight("fresh", 3, 1);

        assert_eq!(report.evicted, vec!["heavy"]);
        assert_eq!(cache.get_cloned(&"heavy"), None);
        assert_eq!(cache.get_cloned(&"light"), Some(2));
        assert_eq!(cache.get_cloned(&"fresh"), Some(3));
    }

    #[test]
    fn insert_report_exposes_total_weight_and_utilization() {
        let mut cache = BoundedLruCache::new(4);
        let report = cache.insert_with_weight("a", 1, 3);

        assert_eq!(report.evicted, Vec::<&str>::new());
        assert_eq!(report.total_weight, 3);
        assert_eq!(report.utilization, 25);
    }
}
