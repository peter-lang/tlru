use std::collections::HashMap;
use std::fmt;
use std::fmt::Debug;
use std::hash::Hash;
use std::time::Duration;
#[cfg(not(test))]
use std::time::Instant;

#[cfg(test)]
use mock_instant::global::Instant;

use crate::queue::{self, NodePtr, Queue};

pub struct Record<K, V> {
    pub key: K,
    pub value: V,
    pub access: Instant,
}

pub struct TLRUCache<K, V>
where
    K: Clone,
    V: Clone,
{
    expiry: Duration,
    store: HashMap<K, NodePtr<Record<K, V>>>,
    order: Queue<Record<K, V>>,
}

pub struct Iter<'a, T> {
    iter: queue::Iter<'a, T>,
}

impl<'a, T> Iterator for Iter<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }
}

impl<K, V> TLRUCache<K, V>
where
    K: Clone + Eq + Hash,
    V: Clone,
{
    pub fn new(expiry: Duration) -> Self {
        Self {
            expiry,
            store: HashMap::new(),
            order: Queue::new(),
        }
    }

    pub fn insert(&mut self, key: &K, value: V) {
        match self.store.get(key) {
            None => {
                let rec_ptr = self.order.push(Record {
                    key: key.clone(),
                    value,
                    access: Instant::now(),
                });
                self.store.insert(key.clone(), rec_ptr);
            }
            Some(&old) => {
                unsafe {
                    (*old).value.access = Instant::now();
                }
                self.order.remove(old);
                self.order.push_node(old);
            }
        }
    }

    pub fn insert_new<KF>(&mut self, generate_random_key: KF, value: V) -> K
    where
        KF: Fn() -> K,
    {
        let mut key = generate_random_key();
        while self.store.contains_key(&key) {
            key = generate_random_key();
        }
        let rec_ptr = self.order.push(Record {
            key: key.clone(),
            value,
            access: Instant::now(),
        });
        self.store.insert(key.clone(), rec_ptr);
        key
    }

    pub fn fetch(&mut self, key: &K) -> Option<V> {
        match self.store.get(key) {
            None => None,
            Some(&old) => unsafe {
                (*old).value.access = Instant::now();
                self.order.remove(old);
                self.order.push_node(old);
                Some((*old).value.value.clone())
            },
        }
    }

    pub fn remove(&mut self, key: &K) -> Option<V> {
        match self.store.remove(key) {
            None => None,
            Some(old) => unsafe {
                self.order.remove(old);
                let data = Box::from_raw(old);
                Some(data.value.value)
            },
        }
    }

    pub fn vacuum(&mut self) -> &mut Self {
        while let Some(Record { access, .. }) = self.order.peek() {
            if access.elapsed() < self.expiry {
                break;
            }
            let Record { key, .. } = self.order.pop_node().unwrap().value;
            _ = self.store.remove(&key);
        }
        self
    }

    pub fn iter(&self) -> Iter<Record<K, V>> {
        Iter {
            iter: self.order.iter(),
        }
    }
}

impl<K: Debug, V: Debug> Debug for TLRUCache<K, V>
where
    K: Clone,
    V: Clone,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_map()
            .entries(
                self.order
                    .iter()
                    .map(|Record { key, value, access }| (key, (access.elapsed(), value))),
            )
            .finish()
    }
}

#[cfg(test)]
mod test {
    use std::time::Duration;

    use uuid::Uuid;

    use crate::tlru::TLRUCache;

    use mock_instant::global::MockClock;

    #[test]
    fn test_insert() {
        let mut session = TLRUCache::new(Duration::ZERO);
        session.insert_new(|| Uuid::new_v4(), 1);
        session.insert_new(|| Uuid::new_v4(), 2);
        session.insert_new(|| Uuid::new_v4(), 3);

        assert_eq!(
            session.iter().map(|x| x.value).collect::<Vec<_>>(),
            vec![1, 2, 3]
        );
    }

    #[test]
    fn test_fetch() {
        let mut session = TLRUCache::new(Duration::ZERO);
        let k1 = session.insert_new(|| Uuid::new_v4(), 1);
        let k2 = session.insert_new(|| Uuid::new_v4(), 2);
        let k3 = session.insert_new(|| Uuid::new_v4(), 3);
        assert_eq!(session.remove(&k2), Some(2));
        assert_eq!(session.remove(&k2), None);
        assert_eq!(
            session.iter().map(|x| x.value).collect::<Vec<_>>(),
            vec![1, 3]
        );

        assert_eq!(session.fetch(&k1), Some(1));
        assert_eq!(
            session.iter().map(|x| x.value).collect::<Vec<_>>(),
            vec![3, 1]
        );

        assert_eq!(session.fetch(&k3), Some(3));
        assert_eq!(
            session.iter().map(|x| x.value).collect::<Vec<_>>(),
            vec![1, 3]
        );
    }

    #[test]
    fn test_vacuum() {
        MockClock::set_time(Duration::ZERO);
        let mut session = TLRUCache::new(Duration::from_secs(2));
        session.insert_new(|| Uuid::new_v4(), 1);
        MockClock::advance(Duration::from_millis(500));
        let k2 = session.insert_new(|| Uuid::new_v4(), 2);
        MockClock::advance(Duration::from_millis(500));
        session.insert_new(|| Uuid::new_v4(), 3);
        MockClock::advance(Duration::from_millis(500));
        // 1: 0.0, 2: 0.5, 3: 1.0
        session.vacuum().fetch(&k2);
        assert_eq!(
            session.iter().map(|x| x.value).collect::<Vec<_>>(),
            vec![1, 3, 2]
        );

        MockClock::advance(Duration::from_millis(700));
        session.vacuum().insert_new(|| Uuid::new_v4(), 4);
        // 3: 1.0, 2: 1.5, 4: 2.2
        assert_eq!(
            session.iter().map(|x| x.value).collect::<Vec<_>>(),
            vec![3, 2, 4]
        );

        MockClock::advance(Duration::from_millis(1700));
        session.vacuum();
        assert_eq!(session.iter().map(|x| x.value).collect::<Vec<_>>(), vec![4]);

        MockClock::advance(Duration::from_millis(500));
        session.vacuum();
        assert_eq!(
            session.iter().map(|x| x.value).collect::<Vec<_>>(),
            Vec::new()
        );

        MockClock::advance(Duration::from_millis(500));
        session.vacuum();
        assert_eq!(
            session.iter().map(|x| x.value).collect::<Vec<_>>(),
            Vec::new()
        );
    }
}
