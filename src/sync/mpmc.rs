use core::{
    cell::{Cell, RefCell},
    future::Future,
    pin::Pin,
    ptr::NonNull,
    task::{Context, Poll, Waker},
};

use bare_metal::Mutex;

use crate::{
    interrupt,
    linked_list::{LinkedList, LinkedListItem, LinkedListLinks},
};

pub struct Buffer<'b, T> {
    senders: LinkedList<Send<'b, T>>,
    receivers: LinkedList<Receive<'b, T>>,
}

impl<'b, T> Buffer<'b, T> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn sender(&'b self) -> Sender<'b, T> {
        Sender { buffer: self }
    }

    pub fn receiver(&'b self) -> Receiver<'b, T> {
        Receiver { buffer: self }
    }
}

impl<'b, T> Default for Buffer<'b, T> {
    fn default() -> Self {
        Self {
            senders: LinkedList::default(),
            receivers: LinkedList::default(),
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub struct SendError<T>(T);

pub struct Sender<'b, T> {
    buffer: &'b Buffer<'b, T>,
}

impl<'b, T> Sender<'b, T> {
    pub fn send(&self, value: T) -> Send<'b, T> {
        Send {
            buffer: NonNull::from(self.buffer),
            value: Mutex::new(RefCell::new(Some(value))),
            waker: Mutex::new(Cell::new(None)),
            links: LinkedListLinks::default(),
        }
    }
}

pub struct Send<'b, T> {
    buffer: NonNull<Buffer<'b, T>>,
    value: Mutex<RefCell<Option<T>>>,
    waker: Mutex<Cell<Option<Waker>>>,
    links: LinkedListLinks<Self>,
}

impl<'b, T> LinkedListItem for Send<'b, T> {
    fn links(&self) -> &LinkedListLinks<Self> {
        &self.links
    }

    fn list(&self) -> &LinkedList<Self> {
        &unsafe { self.buffer.as_ref() }.senders
    }
}

impl<'b, T> Future for Send<'b, T> {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        interrupt::free(|cs| {
            let this = unsafe { self.get_unchecked_mut() };

            if this.value.borrow(cs).borrow().is_none() {
                this.remove(cs);
                Poll::Ready(())
            } else {
                this.insert_back(cs);

                this.waker.borrow(cs).set(Some(cx.waker().clone()));

                unsafe { this.buffer.as_mut() }
                    .receivers
                    .with_first(cs, |receiver| {
                        if let Some(waker) = receiver.waker.borrow(cs).take() {
                            waker.wake();
                        }
                    });
                Poll::Pending
            }
        })
    }
}

impl<'b, T> Drop for Send<'b, T> {
    fn drop(&mut self) {
        interrupt::free(|cs| self.remove(cs));
    }
}

pub struct Receiver<'b, T> {
    buffer: &'b Buffer<'b, T>,
}

impl<'b, T> Receiver<'b, T> {
    pub fn receive(&self) -> Receive<'b, T> {
        Receive {
            buffer: NonNull::from(self.buffer),
            waker: Mutex::new(Cell::new(None)),
            links: LinkedListLinks::default(),
        }
    }
}

pub struct Receive<'b, T> {
    buffer: NonNull<Buffer<'b, T>>,
    waker: Mutex<Cell<Option<Waker>>>,
    links: LinkedListLinks<Self>,
}

impl<'b, T> LinkedListItem for Receive<'b, T> {
    fn links(&self) -> &LinkedListLinks<Self> {
        &self.links
    }

    fn list(&self) -> &LinkedList<Self> {
        &unsafe { self.buffer.as_ref() }.receivers
    }
}

impl<'b, T> Future for Receive<'b, T> {
    type Output = T;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        interrupt::free(|cs| {
            let this = unsafe { self.get_unchecked_mut() };
            let buffer = unsafe { this.buffer.as_mut() };
            match buffer.senders.with_first(cs, |sender| {
                sender.remove(cs);
                sender
                    .waker
                    .borrow(cs)
                    .replace(None)
                    .expect("Sender has waker")
                    .wake();
                sender
                    .value
                    .borrow(cs)
                    .replace(None)
                    .expect("Sender has value")
            }) {
                Some(value) => {
                    this.remove(cs);
                    Poll::Ready(value)
                }
                None => {
                    this.insert_back(cs);
                    this.waker.borrow(cs).set(Some(cx.waker().clone()));
                    Poll::Pending
                }
            }
        })
    }
}

impl<'b, T> Drop for Receive<'b, T> {
    fn drop(&mut self) {
        interrupt::free(|cs| self.remove(cs));
    }
}
