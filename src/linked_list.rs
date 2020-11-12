use core::ptr::{read_volatile, write_volatile, NonNull};

#[derive(Debug)]
pub struct LinkedListLinks<T, E> {
    pub previous: T,
    pub next: T,
    pub ends: E,
}

#[derive(Debug)]
pub struct LinkedListEnds<T> {
    pub first: T,
    pub last: T,
}

unsafe fn take_volatile<T>(ptr: *mut Option<T>) -> Option<T> {
    let value = read_volatile(ptr);
    write_volatile(ptr, None);
    value
}

pub trait LinkedListItem {
    fn links(
        &self,
    ) -> LinkedListLinks<*const Option<NonNull<Self>>, *const Option<LinkedListEnds<NonNull<Self>>>>;

    fn links_mut(
        &mut self,
    ) -> LinkedListLinks<*mut Option<NonNull<Self>>, *mut Option<LinkedListEnds<NonNull<Self>>>>;

    fn is_in_queue(&self) -> bool {
        let links = self.links();
        unsafe {
            read_volatile(links.previous).is_some()
                || read_volatile(links.next).is_some()
                || read_volatile(links.ends)
                    .map_or(false, |ends| core::ptr::eq(ends.first.as_ptr(), self))
        }
    }

    fn insert_front(&mut self) {
        if self.is_in_queue() {
            return;
        }

        let self_ptr = NonNull::from(self as &Self);

        let links = self.links_mut();

        unsafe {
            match read_volatile(links.ends) {
                Some(mut ends) => {
                    write_volatile(ends.first.as_mut().links_mut().previous, Some(self_ptr));
                    write_volatile(links.next, Some(ends.first));
                    write_volatile(&mut (&mut *links.ends).as_mut().unwrap().first, self_ptr);
                }
                None => {
                    write_volatile(
                        self.links_mut().ends,
                        Some(LinkedListEnds {
                            first: self_ptr,
                            last: self_ptr,
                        }),
                    );
                }
            }
        }
    }

    fn insert_back(&mut self) {
        if self.is_in_queue() {
            return;
        }

        let self_ptr = NonNull::from(self as &Self);

        let links = self.links_mut();

        unsafe {
            match read_volatile(links.ends) {
                Some(mut ends) => {
                    write_volatile(ends.last.as_mut().links_mut().next, Some(self_ptr));
                    write_volatile(links.previous, Some(ends.last));
                    write_volatile(&mut (&mut *links.ends).as_mut().unwrap().last, self_ptr);
                }
                None => {
                    write_volatile(
                        self.links_mut().ends,
                        Some(LinkedListEnds {
                            first: self_ptr,
                            last: self_ptr,
                        }),
                    );
                }
            }
        }
    }

    fn remove(&mut self) {
        let self_ptr = self as *const Self;

        let links = self.links_mut();

        match unsafe { (take_volatile(links.previous), take_volatile(links.next)) } {
            (None, None) => {
                // Possible not queued
                if let Some(ends) = unsafe { read_volatile(links.ends) } {
                    if core::ptr::eq(ends.first.as_ptr(), self_ptr) {
                        unsafe { write_volatile(links.ends, None) };
                    }
                }
            }
            (None, Some(mut next)) => {
                // First in queue
                unsafe {
                    write_volatile(
                        links.ends,
                        Some(LinkedListEnds {
                            first: next,
                            last: read_volatile(links.ends).expect("List is not empty").last,
                        }),
                    );
                    write_volatile(next.as_mut().links_mut().previous, None);
                }
            }
            (Some(mut previous), Some(mut next)) => {
                // In middle of queue

                unsafe {
                    write_volatile(previous.as_mut().links_mut().next, Some(next));
                    write_volatile(next.as_mut().links_mut().previous, Some(previous));
                }
            }
            (Some(mut previous), None) => {
                // Last in queue
                unsafe {
                    write_volatile(
                        links.ends,
                        Some(LinkedListEnds {
                            first: read_volatile(links.ends).expect("List is not empty").first,
                            last: previous,
                        }),
                    );
                    write_volatile(previous.as_mut().links_mut().next, None);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use core::ptr::NonNull;

    use super::*;

    #[derive(Default)]
    struct LinkedList {
        ends: Option<LinkedListEnds<NonNull<Node>>>,
    }

    impl LinkedList {
        fn assert_is_valid(&self) {
            unsafe {
                if let Some(ends) = &self.ends {
                    assert!(ends.first.as_ref().previous.is_none());
                    assert!(ends.last.as_ref().next.is_none());

                    let mut current_node = ends.first;

                    loop {
                        if let Some(previous) = current_node.as_ref().previous {
                            let previous_next = previous.as_ref().next.expect("Node has next node");
                            assert!(core::ptr::eq(current_node.as_ptr(), previous_next.as_ptr()));
                        }

                        if let Some(next) = current_node.as_ref().next {
                            let next_previous =
                                next.as_ref().previous.expect("Node has previous node");
                            assert!(core::ptr::eq(current_node.as_ptr(), next_previous.as_ptr()));

                            current_node = next;
                            continue;
                        }

                        break;
                    }
                }
            }
        }

        fn is_empty(&self) -> bool {
            self.ends.is_none()
        }

        fn contains(&self, node: *const Node) -> bool {
            if let Some(ends) = &self.ends {
                let mut current_node = ends.first;

                loop {
                    if core::ptr::eq(current_node.as_ptr(), node) {
                        return true;
                    }

                    if let Some(next_node) = unsafe { current_node.as_ref().next } {
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

    struct Node {
        list: NonNull<LinkedList>,
        previous: Option<NonNull<Node>>,
        next: Option<NonNull<Node>>,
    }

    impl Node {
        fn new(list: &LinkedList) -> Self {
            Self {
                list: list.into(),
                previous: None,
                next: None,
            }
        }
    }

    impl LinkedListItem for Node {
        fn links(
            &self,
        ) -> LinkedListLinks<
            *const Option<NonNull<Self>>,
            *const Option<LinkedListEnds<NonNull<Self>>>,
        > {
            LinkedListLinks {
                previous: &self.previous,
                next: &self.next,
                ends: unsafe { &self.list.as_ref().ends },
            }
        }

        fn links_mut(
            &mut self,
        ) -> LinkedListLinks<*mut Option<NonNull<Self>>, *mut Option<LinkedListEnds<NonNull<Self>>>>
        {
            LinkedListLinks {
                previous: &mut self.previous,
                next: &mut self.next,
                ends: unsafe { &mut self.list.as_mut().ends },
            }
        }
    }

    #[test]
    fn empty_list_is_valid() {
        let list = LinkedList::default();
        list.assert_is_valid();
        assert!(list.is_empty());
    }

    #[test]
    fn singleton_insert_front_is_valid() {
        let list = LinkedList::default();

        let mut node = Node::new(&list);
        node.insert_front();

        list.assert_is_valid();
        assert!(list.contains(&node));
    }

    #[test]
    fn singleton_insert_back_is_valid() {
        let list = LinkedList::default();

        let mut node = Node::new(&list);
        node.insert_back();

        list.assert_is_valid();
        assert!(list.contains(&node));
    }

    #[test]
    fn list_a_b_is_valid() {
        let list = LinkedList::default();

        let mut node_a = Node::new(&list);
        let mut node_b = Node::new(&list);

        node_a.insert_back();
        node_b.insert_back();

        list.assert_is_valid();
        assert!(list.contains(&node_a));
        assert!(list.contains(&node_b));

        assert!(node_a.next.is_some());
        assert!(core::ptr::eq(node_a.next.unwrap().as_ptr(), &node_b));
    }

    #[test]
    fn list_b_a_is_valid() {
        let list = LinkedList::default();

        let mut node_a = Node::new(&list);
        let mut node_b = Node::new(&list);

        node_a.insert_front();
        node_b.insert_front();

        list.assert_is_valid();
        assert!(list.contains(&node_a));
        assert!(list.contains(&node_b));

        assert!(node_b.next.is_some());
        assert!(core::ptr::eq(node_b.next.unwrap().as_ptr(), &node_a));
    }

    fn run_triple_test(remove_order: [usize; 3]) {
        let list = LinkedList::default();

        let mut nodes = [Node::new(&list), Node::new(&list), Node::new(&list)];

        for node in nodes.iter_mut() {
            node.insert_back();
        }

        for node in nodes.iter_mut() {
            assert!(list.contains(node));
        }

        nodes[remove_order[0]].remove();

        assert!(!list.contains(&nodes[remove_order[0]]));
        assert!(list.contains(&nodes[remove_order[1]]));
        assert!(list.contains(&nodes[remove_order[2]]));

        assert!(!nodes[remove_order[0]].is_in_queue());
        assert!(nodes[remove_order[1]].is_in_queue());
        assert!(nodes[remove_order[2]].is_in_queue());

        nodes[remove_order[1]].remove();

        assert!(!list.contains(&nodes[remove_order[0]]));
        assert!(!list.contains(&nodes[remove_order[1]]));
        assert!(list.contains(&nodes[remove_order[2]]));

        assert!(!nodes[remove_order[0]].is_in_queue());
        assert!(!nodes[remove_order[1]].is_in_queue());
        assert!(nodes[remove_order[2]].is_in_queue());

        nodes[remove_order[2]].remove();

        assert!(!list.contains(&nodes[remove_order[0]]));
        assert!(!list.contains(&nodes[remove_order[1]]));
        assert!(!list.contains(&nodes[remove_order[2]]));

        assert!(!nodes[remove_order[0]].is_in_queue());
        assert!(!nodes[remove_order[1]].is_in_queue());
        assert!(!nodes[remove_order[2]].is_in_queue());
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
