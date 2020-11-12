use core::ptr::NonNull;

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

pub trait LinkedListItem {
    fn links(
        &self,
    ) -> LinkedListLinks<&Option<NonNull<Self>>, &Option<LinkedListEnds<NonNull<Self>>>>;

    fn links_mut(
        &mut self,
    ) -> LinkedListLinks<&mut Option<NonNull<Self>>, &mut Option<LinkedListEnds<NonNull<Self>>>>;

    fn is_in_queue(&self) -> bool {
        let links = self.links();
        links.previous.is_some()
            || links.next.is_some()
            || self
                .links()
                .ends
                .as_ref()
                .map_or(false, |ends| core::ptr::eq(ends.first.as_ptr(), self))
    }

    fn insert_front(&mut self) {
        if self.is_in_queue() {
            return;
        }

        let self_ptr = NonNull::from(self as &Self);

        let links = self.links_mut();

        unsafe {
            match links.ends {
                Some(ends) => {
                    *(ends.first.as_mut().links_mut().previous) = Some(self_ptr);
                    *(links.next) = Some(ends.first);
                    ends.first = self_ptr;
                }
                None => {
                    *(self.links_mut().ends) = Some(LinkedListEnds {
                        first: self_ptr,
                        last: self_ptr,
                    })
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
            match links.ends {
                Some(ends) => {
                    *(ends.last.as_mut().links_mut().next) = Some(self_ptr);
                    *(links.previous) = Some(ends.last);
                    ends.last = self_ptr;
                }
                None => {
                    *(self.links_mut().ends) = Some(LinkedListEnds {
                        first: self_ptr,
                        last: self_ptr,
                    })
                }
            }
        }
    }

    fn remove(&mut self) {
        let self_ptr = self as *const Self;

        let links = self.links_mut();

        match (links.previous.take(), links.next.take()) {
            (None, None) => {
                // Possible not queued
                if let Some(ends) = links.ends.as_mut() {
                    if core::ptr::eq(ends.first.as_ptr(), self_ptr) {
                        *(links.ends) = None;
                    }
                }
            }
            (None, Some(mut next)) => {
                // First in queue

                links.ends.as_mut().expect("List is not empty").first = next;
                *(unsafe { next.as_mut() }.links_mut().previous) = None;
            }
            (Some(mut previous), Some(mut next)) => {
                // In middle of queue

                *(unsafe { previous.as_mut() }.links_mut().next) = Some(next);
                *(unsafe { next.as_mut() }.links_mut().previous) = Some(previous);
            }
            (Some(mut previous), None) => {
                // Last in queue

                links.ends.as_mut().expect("List is not empty").last = previous;

                *(unsafe { previous.as_mut() }.links_mut().next) = None;
            }
        }
    }
}
