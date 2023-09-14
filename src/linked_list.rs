use bare_metal::CriticalSection;

use crate::{mutex::Mutex, non_null::NonNull};

struct LinkedListLink<T>(Mutex<Option<NonNull<T>>>);

impl<T> LinkedListLink<T> {
    fn get(&self, cs: &CriticalSection) -> Option<NonNull<T>> {
        self.0.get(cs)
    }

    fn set(&self, cs: &CriticalSection, value: Option<NonNull<T>>) {
        self.0.set(cs, value)
    }

    fn take(&self, cs: &CriticalSection) -> Option<NonNull<T>> {
        self.0.take(cs)
    }
}

impl<T> Default for LinkedListLink<T> {
    fn default() -> Self {
        Self(Mutex::default())
    }
}

pub struct LinkedListLinks<T> {
    previous: LinkedListLink<T>,
    next: LinkedListLink<T>,
}

impl<T> Default for LinkedListLinks<T> {
    fn default() -> Self {
        Self {
            previous: LinkedListLink::default(),
            next: LinkedListLink::default(),
        }
    }
}

struct LinkedListCore<T> {
    first: NonNull<T>,
    last: NonNull<T>,
}

impl<T> Clone for LinkedListCore<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for LinkedListCore<T> {}

pub struct LinkedList<T> {
    core: Mutex<Option<LinkedListCore<T>>>,
}

impl<T> LinkedList<T> {
    pub fn with_first<F, R>(&self, cs: &CriticalSection, f: F) -> Option<R>
    where
        F: FnOnce(&T) -> R,
    {
        self.core
            .get(cs)
            .map(|core| f(unsafe { core.first.as_ref() }))
    }
}

impl<T: LinkedListItem> LinkedList<T> {
    pub fn pop_first(&self, cs: &CriticalSection) -> Option<&T> {
        self.core.get(cs).map(|core| {
            let first = unsafe { core.first.as_ref() };
            first.remove(cs);
            first
        })
    }
}

impl<T> Default for LinkedList<T> {
    fn default() -> Self {
        Self {
            core: Mutex::default(),
        }
    }
}

pub trait LinkedListItem: Sized {
    fn links(&self) -> &LinkedListLinks<Self>;

    fn list(&self) -> &LinkedList<Self>;

    fn is_in_queue(&self, cs: &CriticalSection) -> bool {
        let links = self.links();
        links.previous.get(cs).is_some()
            || links.next.get(cs).is_some()
            || self
                .list()
                .core
                .get(cs)
                .map_or(false, |core| core::ptr::eq(core.first.as_ptr(), self))
    }

    fn insert_front(&self, cs: &CriticalSection) -> &Self {
        if self.is_in_queue(cs) {
            return self;
        }

        let self_ptr = NonNull::new(self);

        let list = &self.list().core;

        match list.get(cs) {
            Some(mut core) => {
                self.set_next(cs, Some(core.first));
                unsafe { core.first.as_ref() }.set_previous(cs, Some(self_ptr));
                core.first = self_ptr;
                list.set(cs, Some(core));
            }
            None => list.set(
                cs,
                Some(LinkedListCore {
                    first: self_ptr,
                    last: self_ptr,
                }),
            ),
        }

        self
    }

    fn insert_back(&self, cs: &CriticalSection) -> &Self {
        if self.is_in_queue(cs) {
            return self;
        }

        let self_ptr = NonNull::new(self);

        let list = &self.list().core;

        match list.get(cs) {
            Some(mut core) => {
                self.set_previous(cs, Some(core.last));
                unsafe { core.last.as_ref() }.set_next(cs, Some(self_ptr));
                core.last = self_ptr;
                list.set(cs, Some(core));
            }
            None => list.set(
                cs,
                Some(LinkedListCore {
                    first: self_ptr,
                    last: self_ptr,
                }),
            ),
        }

        self
    }

    fn remove(&self, cs: &CriticalSection) {
        let self_ptr = self as *const Self;

        let links = self.links();
        let list = &self.list().core;

        match (links.previous.take(cs), links.next.take(cs)) {
            (None, None) => {
                // Possible not queued
                if let Some(ends) = list.get(cs) {
                    if core::ptr::eq(ends.first.as_ptr(), self_ptr) {
                        list.set(cs, None);
                    }
                }
            }
            (None, Some(next)) => {
                // First in queue
                let list = &self.list().core;
                list.set(
                    cs,
                    Some(LinkedListCore {
                        first: next,
                        last: list.get(cs).expect("List is not empty").last,
                    }),
                );
                unsafe { next.as_ref() }.set_previous(cs, None);
            }
            (Some(previous), Some(next)) => {
                // In middle of queue
                unsafe { previous.as_ref() }.set_next(cs, Some(next));
                unsafe { next.as_ref() }.set_previous(cs, Some(previous));
            }
            (Some(previous), None) => {
                // Last in queue

                let list = &self.list().core;
                list.set(
                    cs,
                    Some(LinkedListCore {
                        first: list.get(cs).expect("List is not empty").first,
                        last: previous,
                    }),
                );
                unsafe { previous.as_ref() }.set_next(cs, None);
            }
        }
    }
}

trait LinkedListItemUtil: LinkedListItem {
    fn set_previous(&self, cs: &CriticalSection, previous: Option<NonNull<Self>>) {
        self.links().previous.set(cs, previous);
    }

    fn set_next(&self, cs: &CriticalSection, next: Option<NonNull<Self>>) {
        self.links().next.set(cs, next);
    }
}

impl<T: LinkedListItem> LinkedListItemUtil for T {}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::interrupt;

    #[derive(Default)]
    struct TestLinkedList<'a> {
        list: LinkedList<Node<'a>>,
    }

    impl<'a> TestLinkedList<'a> {
        fn assert_is_valid(&self) {
            interrupt::free(|cs| unsafe {
                if let Some(ends) = self.list.core.get(cs) {
                    assert!(ends.first.as_ref().links.previous.get(cs).is_none());
                    assert!(ends.last.as_ref().links.next.get(cs).is_none());

                    let mut current_node = ends.first;

                    loop {
                        if let Some(previous) = current_node.as_ref().links.previous.get(cs) {
                            let previous_next = previous
                                .as_ref()
                                .links
                                .next
                                .get(cs)
                                .expect("Node has next node");
                            assert!(core::ptr::eq(current_node.as_ptr(), previous_next.as_ptr()));
                        }

                        if let Some(next) = current_node.as_ref().links.next.get(cs) {
                            let next_previous = next
                                .as_ref()
                                .links
                                .previous
                                .get(cs)
                                .expect("Node has previous node");
                            assert!(core::ptr::eq(current_node.as_ptr(), next_previous.as_ptr()));

                            current_node = next;
                            continue;
                        }

                        break;
                    }
                }
            })
        }

        fn is_empty(&self) -> bool {
            interrupt::free(|cs| self.list.core.get(cs).is_none())
        }

        fn contains(&self, node: *const Node<'a>, cs: &CriticalSection) -> bool {
            if let Some(ends) = self.list.core.get(cs) {
                let mut current_node = ends.first;

                loop {
                    if core::ptr::eq(current_node.as_ptr(), node) {
                        return true;
                    }

                    if let Some(next_node) = unsafe { current_node.as_ref() }.links.next.get(cs) {
                        current_node = next_node;
                    } else {
                        return false;
                    }
                }
            } else {
                false
            }
        }
    }

    struct Node<'a> {
        list: &'a TestLinkedList<'a>,
        links: LinkedListLinks<Self>,
    }

    impl<'a> Node<'a> {
        fn new(list: &'a TestLinkedList<'a>) -> Self {
            Self {
                list,
                links: LinkedListLinks::default(),
            }
        }
    }

    impl<'a> LinkedListItem for Node<'a> {
        fn links(&self) -> &LinkedListLinks<Self> {
            &self.links
        }

        fn list(&self) -> &LinkedList<Self> {
            &self.list.list
        }
    }

    #[test]
    fn empty_list_is_valid() {
        let list = TestLinkedList::default();
        list.assert_is_valid();
        assert!(list.is_empty());
    }

    #[test]
    fn singleton_insert_front_is_valid() {
        interrupt::free(|cs| {
            let list = TestLinkedList::default();

            let node = Node::new(&list);
            node.insert_front(cs);

            list.assert_is_valid();
            assert!(list.contains(&node, cs));
        });
    }

    #[test]
    fn singleton_insert_back_is_valid() {
        interrupt::free(|cs| {
            let list = TestLinkedList::default();

            let node = Node::new(&list);
            node.insert_back(cs);

            list.assert_is_valid();
            assert!(list.contains(&node, cs));
        });
    }

    #[test]
    fn list_a_b_is_valid() {
        interrupt::free(|cs| {
            let list = TestLinkedList::default();

            let node_a = Node::new(&list);
            let node_b = Node::new(&list);

            node_a.insert_back(cs);
            node_b.insert_back(cs);

            list.assert_is_valid();
            assert!(list.contains(&node_a, cs));
            assert!(list.contains(&node_b, cs));

            assert!(node_a.links.next.get(cs).is_some());
            assert!(core::ptr::eq(
                node_a.links.next.get(cs).unwrap().as_ptr(),
                &node_b
            ));
        });
    }

    #[test]
    fn list_b_a_is_valid() {
        interrupt::free(|cs| {
            let list = TestLinkedList::default();

            let node_a = Node::new(&list);
            let node_b = Node::new(&list);

            node_a.insert_front(cs);
            node_b.insert_front(cs);

            list.assert_is_valid();
            assert!(list.contains(&node_a, cs));
            assert!(list.contains(&node_b, cs));

            assert!(node_b.links.next.get(cs).is_some());
            assert!(core::ptr::eq(
                node_b.links.next.get(cs).unwrap().as_ptr(),
                &node_a
            ));
        });
    }

    fn run_triple_test(remove_order: [usize; 3]) {
        interrupt::free(|cs| {
            let list = TestLinkedList::default();

            let mut nodes = [Node::new(&list), Node::new(&list), Node::new(&list)];

            for node in nodes.iter_mut() {
                node.insert_back(cs);
            }

            for node in nodes.iter() {
                assert!(list.contains(node, cs));
            }

            nodes[remove_order[0]].remove(cs);

            assert!(!list.contains(&nodes[remove_order[0]], cs));
            assert!(list.contains(&nodes[remove_order[1]], cs));
            assert!(list.contains(&nodes[remove_order[2]], cs));

            assert!(!nodes[remove_order[0]].is_in_queue(cs));
            assert!(nodes[remove_order[1]].is_in_queue(cs));
            assert!(nodes[remove_order[2]].is_in_queue(cs));

            nodes[remove_order[1]].remove(cs);

            assert!(!list.contains(&nodes[remove_order[0]], cs));
            assert!(!list.contains(&nodes[remove_order[1]], cs));
            assert!(list.contains(&nodes[remove_order[2]], cs));

            assert!(!nodes[remove_order[0]].is_in_queue(cs));
            assert!(!nodes[remove_order[1]].is_in_queue(cs));
            assert!(nodes[remove_order[2]].is_in_queue(cs));

            nodes[remove_order[2]].remove(cs);

            assert!(!list.contains(&nodes[remove_order[0]], cs));
            assert!(!list.contains(&nodes[remove_order[1]], cs));
            assert!(!list.contains(&nodes[remove_order[2]], cs));

            assert!(!nodes[remove_order[0]].is_in_queue(cs));
            assert!(!nodes[remove_order[1]].is_in_queue(cs));
            assert!(!nodes[remove_order[2]].is_in_queue(cs));
        });
    }

    #[test]
    fn triple_list_is_valid_012() {
        run_triple_test([0, 1, 2]);
    }

    #[test]
    fn triple_list_is_valid_021() {
        run_triple_test([0, 2, 1]);
    }

    #[test]
    fn triple_list_is_valid_102() {
        run_triple_test([1, 0, 2]);
    }

    #[test]
    fn triple_list_is_valid_120() {
        run_triple_test([1, 2, 0]);
    }

    #[test]
    fn triple_list_is_valid_201() {
        run_triple_test([2, 0, 1]);
    }

    #[test]
    fn triple_list_is_valid_210() {
        run_triple_test([2, 1, 0]);
    }
}
