use core::{
    future::Future,
    pin::Pin,
    task::{Context, Poll, Waker},
};

use crate::{
    linked_list::{LinkedList, LinkedListItem, LinkedListLinks},
    mutex::Mutex,
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
    #[must_use = "Send does nothing until it is polled or awaited"]
    pub fn send(&self, value: T) -> Send<'b, T> {
        Send {
            buffer: self.buffer,
            value: Mutex::new(Some(value)),
            waker: Mutex::new(None),
            links: LinkedListLinks::default(),
        }
    }
}

pub struct Send<'b, T> {
    buffer: &'b Buffer<'b, T>,
    value: Mutex<Option<T>>,
    waker: Mutex<Option<Waker>>,
    links: LinkedListLinks<Self>,
}

impl<'b, T> LinkedListItem for Send<'b, T> {
    fn links(&self) -> &LinkedListLinks<Self> {
        &self.links
    }

    fn list(&self) -> &LinkedList<Self> {
        &self.buffer.senders
    }
}

impl<'b, T> Future for Send<'b, T> {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        critical_section::with(|cs| {
            let this = unsafe { self.get_unchecked_mut() };

            if this.value.has_none(cs) {
                this.remove(cs);
                Poll::Ready(())
            } else {
                this.insert_back(cs);

                this.waker.set(cs, Some(cx.waker().clone()));

                this.buffer.receivers.with_first(cs, |receiver| {
                    if let Some(waker) = receiver.waker.take(cs) {
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
        critical_section::with(|cs| self.remove(cs));
    }
}

pub struct Receiver<'b, T> {
    buffer: &'b Buffer<'b, T>,
}

impl<'b, T> Receiver<'b, T> {
    #[must_use = "Receive does nothing until it is polled or awaited"]
    pub fn receive(&self) -> Receive<'b, T> {
        Receive {
            buffer: self.buffer,
            waker: Mutex::new(None),
            links: LinkedListLinks::default(),
        }
    }
}

pub struct Receive<'b, T> {
    buffer: &'b Buffer<'b, T>,
    waker: Mutex<Option<Waker>>,
    links: LinkedListLinks<Self>,
}

impl<'b, T> LinkedListItem for Receive<'b, T> {
    fn links(&self) -> &LinkedListLinks<Self> {
        &self.links
    }

    fn list(&self) -> &LinkedList<Self> {
        &self.buffer.receivers
    }
}

impl<'b, T> Future for Receive<'b, T> {
    type Output = T;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        critical_section::with(|cs| {
            let this = unsafe { self.get_unchecked_mut() };

            let sender_value = this.buffer.senders.with_first(cs, |sender| {
                sender.remove(cs);
                sender.waker.take(cs).expect("Sender has waker").wake();
                sender.value.take(cs).expect("Sender has value")
            });

            if let Some(value) = sender_value {
                this.remove(cs);
                Poll::Ready(value)
            } else {
                this.insert_back(cs);
                this.waker.set(cs, Some(cx.waker().clone()));
                Poll::Pending
            }
        })
    }
}

impl<'b, T> Drop for Receive<'b, T> {
    fn drop(&mut self) {
        critical_section::with(|cs| self.remove(cs));
    }
}
