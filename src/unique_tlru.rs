use crate::tlru::{self, Record, TLRUCache};
use std::collections::HashMap;
use std::hash::Hash;
use std::time::Duration;

pub trait Key {
    type K: Eq + PartialEq + Hash;
    fn id(&self) -> Self::K;
}

pub struct UniqueTLRUCache<K, V>
where
    K: Clone,
    V: Clone + Key,
{
    value_ids: HashMap<V::K, K>,
    cache: TLRUCache<K, V>,
}

pub struct Iter<'a, T> {
    iter: tlru::Iter<'a, T>,
}

impl<'a, T> Iterator for Iter<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }
}

impl<K: Clone + Eq + Hash, V: Clone + Key> UniqueTLRUCache<K, V> {
    pub fn new(expiry: Duration) -> Self {
        Self {
            value_ids: HashMap::new(),
            cache: TLRUCache::new(expiry),
        }
    }

    pub fn insert_new<KF>(&mut self, generate_random_key: KF, value: V) -> K
    where
        KF: Fn() -> K,
    {
        let value_id = value.id();
        if let Some(existing_id) = self.value_ids.get(&value_id) {
            if let Some(_) = self.cache.fetch(existing_id) {
                return existing_id.clone();
            } else {
                self.value_ids.remove(&value_id);
            }
        }

        let key = self.cache.insert_new(generate_random_key, value);
        self.value_ids.insert(value_id, key.clone());
        key
    }

    pub fn fetch(&mut self, key: &K) -> Option<V> {
        self.cache.fetch(key)
    }

    pub fn remove(&mut self, key: &K) -> Option<V> {
        self.cache.remove(key).map(|val| {
            self.value_ids.remove(&val.id());
            val
        })
    }

    pub fn remove_value(&mut self, value: &V) -> Option<V> {
        self.value_ids
            .remove(&value.id())
            .and_then(|key| self.cache.remove(&key))
    }

    pub fn vacuum(&mut self) -> &mut Self {
        self.cache.vacuum_callback(|rec| {
            let value_id = rec.value.id();
            self.value_ids.remove(&value_id);
        });
        self
    }

    pub fn iter(&self) -> Iter<Record<K, V>> {
        Iter {
            iter: self.cache.iter(),
        }
    }
}

#[cfg(test)]
mod test {
    use std::time::Duration;

    use uuid::Uuid;

    use super::{Key, UniqueTLRUCache};

    #[derive(Clone)]
    struct MyVal(i32);

    impl Key for MyVal {
        type K = i32;

        fn id(&self) -> Self::K {
            self.0
        }
    }

    #[test]
    fn test_insert() {
        let mut session = UniqueTLRUCache::new(Duration::ZERO);
        let k1 = session.insert_new(|| Uuid::new_v4(), MyVal(1));
        let k2 = session.insert_new(|| Uuid::new_v4(), MyVal(2));
        let k3 = session.insert_new(|| Uuid::new_v4(), MyVal(1));

        assert_eq!(k1, k3);
        assert_ne!(k1, k2);

        assert_eq!(
            session.iter().map(|x| x.value.0).collect::<Vec<_>>(),
            vec![2, 1]
        );
    }
}
