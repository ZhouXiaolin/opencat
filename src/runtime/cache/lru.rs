use std::{
    collections::{HashMap, VecDeque},
    hash::Hash,
};

#[derive(Clone, Debug)]
pub struct BoundedLruCache<K, V> {
    capacity: usize,
    map: HashMap<K, V>,
    order: VecDeque<K>,
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

    pub fn insert(&mut self, key: K, value: V) {
        if self.capacity == 0 {
            return;
        }

        let key_for_order = key.clone();
        let existed = self.map.insert(key, value).is_some();
        self.touch_owned(key_for_order);

        if !existed {
            self.evict_if_needed();
        }
    }

    fn evict_if_needed(&mut self) {
        while self.map.len() > self.capacity {
            if let Some(oldest) = self.order.pop_front() {
                self.map.remove(&oldest);
            } else {
                break;
            }
        }
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
}
