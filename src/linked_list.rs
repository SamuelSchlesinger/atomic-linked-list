use std::ptr::null_mut;

use crossbeam_epoch::{Atomic, Guard, Shared};

pub struct LinkedList<T> {
    head: Atomic<Node<T>>,
}

unsafe impl<T> Sync for LinkedList<T> {}
unsafe impl<T> Send for LinkedList<T> {}

struct Node<T> {
    item: T,
    next: *mut Node<T>,
}

impl<T> LinkedList<T> {
    pub fn new() -> Self {
        LinkedList {
            head: Atomic::null(),
        }
    }

    pub fn push<'g>(&self, item: T, guard: &'g Guard) {
        let new_head = Box::into_raw(Box::new(Node {
            item,
            next: null_mut(),
        })) as *mut Node<T>;
        loop {
            let current_head = self.head.load(std::sync::atomic::Ordering::Relaxed, guard);

            unsafe { (*new_head).next = current_head.as_raw() as *mut Node<T> }

            if self
                .head
                .compare_exchange_weak(
                    current_head,
                    Shared::from(new_head as *const Node<T>),
                    std::sync::atomic::Ordering::Relaxed,
                    std::sync::atomic::Ordering::Relaxed,
                    guard,
                )
                .is_ok()
            {
                break;
            }
        }
    }

    pub fn pop<'g>(&self, guard: &'g Guard) -> Option<&'g T> {
        loop {
            let current_head = self.head.load(std::sync::atomic::Ordering::Relaxed, guard);

            if current_head.is_null() {
                return None;
            }

            if self
                .head
                .compare_exchange_weak(
                    current_head,
                    Shared::from(unsafe { current_head.deref() }.next as *const Node<T>),
                    std::sync::atomic::Ordering::Relaxed,
                    std::sync::atomic::Ordering::Relaxed,
                    guard,
                )
                .is_ok()
            {
                unsafe { guard.defer_destroy(current_head) }
                return Some(&unsafe { current_head.deref() }.item);
            }
        }
    }
}

#[test]
fn linked_list_test() {
    use std::{
        sync::{atomic::AtomicUsize, Arc},
        thread::spawn,
    };

    let list = Arc::new(LinkedList::new());

    let t1 = spawn({
        let list = list.clone();
        move || {
            let guard = crossbeam_epoch::pin();
            for _ in 0..1000 {
                list.push(1, &guard);
            }
        }
    });

    let t2 = spawn({
        let list = list.clone();
        move || {
            let guard = crossbeam_epoch::pin();
            for _ in 0..1000 {
                list.push(1, &guard);
            }
        }
    });
    t1.join().unwrap();
    t2.join().unwrap();

    let x = Arc::new(AtomicUsize::new(0));

    let t1 = spawn({
        let x = x.clone();
        let list = list.clone();
        move || {
            let guard = crossbeam_epoch::pin();
            for _ in 0..1000 {
                if let Some(i) = list.pop(&guard) {
                    x.fetch_add(*i as usize, std::sync::atomic::Ordering::Relaxed);
                }
            }
        }
    });
    let t2 = spawn({
        let x = x.clone();
        let list = list.clone();
        move || {
            let guard = crossbeam_epoch::pin();
            for _ in 0..1000 {
                if let Some(i) = list.pop(&guard) {
                    x.fetch_add(*i as usize, std::sync::atomic::Ordering::Relaxed);
                }
            }
        }
    });
    t1.join().unwrap();
    t2.join().unwrap();

    assert_eq!(x.load(std::sync::atomic::Ordering::SeqCst), 2000);

    let t1 = spawn({
        let list = list.clone();
        move || {
            let guard = crossbeam_epoch::pin();
            for _ in 0..1000 {
                list.push(1, &guard);
            }
        }
    });

    let t2 = spawn({
        let x = x.clone();
        let list = list.clone();
        move || {
            let guard = crossbeam_epoch::pin();
            loop {
                if let Some(i) = list.pop(&guard) {
                    let sum = x.fetch_add(*i as usize, std::sync::atomic::Ordering::Relaxed);
                    if sum == 2999 {
                        break;
                    }
                }
            }
        }
    });

    t1.join().unwrap();
    t2.join().unwrap();
}
