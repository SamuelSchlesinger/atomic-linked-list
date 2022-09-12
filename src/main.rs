mod linked_list;

use std::{sync::Arc, thread};

use linked_list::*;

fn main() {
    const N: usize = 1000;
    const M: usize = 1000;

    let linked_list: Arc<LinkedList<usize>> = Arc::new(LinkedList::new());
    let write_closure = || {
        let guard = crossbeam_epoch::pin();
        for _ in 0..N {
            linked_list.push(1, &guard);
        }
    };
    let read_closure = || {
        let guard = crossbeam_epoch::pin();
        let mut sum = 0;
        loop {
            if sum >= N {
                break;
            }
            if let Some(i) = linked_list.pop(&guard) {
                sum += *i;
            }
        }
    };

    thread::scope(|scope| {
        for _ in 0..M {
            scope.spawn(read_closure);
        }
        for _ in 0..M {
            scope.spawn(write_closure);
        }
    });

    assert_eq!(linked_list.pop(&crossbeam_epoch::pin()), None);
}
