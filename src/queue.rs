use std::fmt::Debug;
use std::marker::PhantomData;
use std::{fmt, ptr};

pub type NodePtr<T> = *mut Node<T>;

pub struct Node<T> {
    pub value: T,
    prev: NodePtr<T>,
    next: NodePtr<T>,
}

pub struct Queue<T> {
    head: NodePtr<T>,
    tail: NodePtr<T>,
    _pd: PhantomData<T>,
}

impl<T> Queue<T> {
    pub fn new() -> Self {
        Self {
            head: ptr::null_mut(),
            tail: ptr::null_mut(),
            _pd: PhantomData,
        }
    }

    pub fn push(&mut self, value: T) -> NodePtr<T> {
        let new_tail = Box::into_raw(Box::new(Node {
            value,
            prev: ptr::null_mut(),
            next: ptr::null_mut(),
        }));
        self.push_node(new_tail);
        new_tail
    }

    pub fn push_node(&mut self, new_tail: NodePtr<T>) {
        unsafe {
            if !self.tail.is_null() {
                (*self.tail).next = new_tail;
                (*new_tail).prev = self.tail;
            } else {
                self.head = new_tail;
            }
            self.tail = new_tail;
        }
    }

    pub fn peek(&self) -> Option<&T> {
        unsafe { self.head.as_ref().map(|node| &node.value) }
    }

    pub fn pop_node(&mut self) -> Option<Box<Node<T>>> {
        unsafe {
            if self.head.is_null() {
                None
            } else {
                let head = Box::from_raw(self.head);
                self.head = head.next;

                if self.head.is_null() {
                    self.tail = ptr::null_mut();
                }

                Some(head)
            }
        }
    }

    pub fn remove(&mut self, elem: NodePtr<T>) {
        unsafe {
            if !(*elem).prev.is_null() {
                (*(*elem).prev).next = (*elem).next;
            }
            if !(*elem).next.is_null() {
                (*(*elem).next).prev = (*elem).prev;
            }
            if self.tail == elem {
                self.tail = (*elem).prev;
            }
            if self.head == elem {
                self.head = (*elem).next;
            }
            (*elem).prev = ptr::null_mut();
            (*elem).next = ptr::null_mut();
        }
    }

    pub fn iter(&self) -> Iter<'_, T> {
        unsafe {
            Iter {
                next: self.head.as_ref(),
            }
        }
    }
}

impl<T> Drop for Queue<T> {
    fn drop(&mut self) {
        while let Some(_) = self.pop_node() {}
    }
}

pub struct Iter<'a, T> {
    next: Option<&'a Node<T>>,
}

impl<'a, T> Iterator for Iter<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        unsafe {
            self.next.map(|node| {
                self.next = node.next.as_ref();
                &node.value
            })
        }
    }
}

impl<T: Debug> Debug for Queue<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}

unsafe impl<T: Send> Send for Queue<T> {}
unsafe impl<T: Sync> Sync for Queue<T> {}

#[cfg(test)]
mod test {
    use super::Queue;

    #[test]
    fn test_send_sync() {
        fn is_send<T: Send>() {}
        fn is_sync<T: Sync>() {}

        is_send::<Queue<i32>>();
        is_sync::<Queue<i32>>();
    }

    #[test]
    fn test_push() {
        let mut list = Queue::new();
        list.push(1);
        list.push(2);
        list.push(3);

        assert_eq!(list.iter().map(|x| *x).collect::<Vec<_>>(), vec![1, 2, 3]);
    }

    #[test]
    fn test_move_to_end() {
        let mut list = Queue::new();
        let el1 = list.push(1);
        let el2 = list.push(2);

        list.remove(el1);
        list.push_node(el1);

        assert_eq!(list.iter().map(|x| *x).collect::<Vec<_>>(), vec![2, 1]);

        list.remove(el2);
        list.push_node(el2);

        assert_eq!(list.iter().map(|x| *x).collect::<Vec<_>>(), vec![1, 2]);
    }

    #[test]
    fn test_remove_front() {
        let mut list = Queue::new();
        let el = list.push(1);
        list.push(2);
        list.push(3);
        list.push(4);

        list.remove(el);

        assert_eq!(list.iter().map(|x| *x).collect::<Vec<_>>(), vec![2, 3, 4]);
    }

    #[test]
    fn test_remove_mid() {
        let mut list = Queue::new();
        list.push(1);
        let el1 = list.push(2);
        let el2 = list.push(3);
        list.push(4);

        list.remove(el2);
        list.remove(el1);

        assert_eq!(list.iter().map(|x| *x).collect::<Vec<_>>(), vec![1, 4]);
    }

    #[test]
    fn test_remove_back() {
        let mut list = Queue::new();
        list.push(1);
        list.push(2);
        list.push(3);
        let el = list.push(4);

        list.remove(el);

        assert_eq!(list.iter().map(|x| *x).collect::<Vec<_>>(), vec![1, 2, 3]);
    }

    #[test]
    fn test_pop() {
        let mut list = Queue::new();
        list.push(1);
        list.push(2);
        list.push(3);

        assert!(list.pop_node().is_some());
        assert_eq!(list.iter().map(|x| *x).collect::<Vec<_>>(), vec![2, 3]);

        assert!(list.pop_node().is_some());
        assert_eq!(list.iter().map(|x| *x).collect::<Vec<_>>(), vec![3]);

        assert!(list.pop_node().is_some());
        assert_eq!(list.iter().map(|x| *x).collect::<Vec<_>>(), Vec::new());

        assert!(list.pop_node().is_none());
        assert_eq!(list.iter().map(|x| *x).collect::<Vec<_>>(), Vec::new());
    }
}
