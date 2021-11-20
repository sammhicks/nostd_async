use std::{
    cell::Cell,
    future::Future,
    pin::Pin,
    task::{Context, Poll, Waker},
};

struct SingleMessage<T> {
    value: Cell<Option<T>>,
    waker: Cell<Option<Waker>>,
}

impl<T> SingleMessage<T> {
    fn new() -> Self {
        Self {
            value: Cell::new(None),
            waker: Cell::new(None),
        }
    }

    fn send(&self, value: T) {
        if self.value.replace(Some(value)).is_some() {
            panic!("Already sending");
        }
        if let Some(waker) = self.waker.take() {
            waker.wake();
        }
    }

    fn recv(&self) -> RecvSingleMessage<'_, T> {
        RecvSingleMessage(self)
    }
}

struct RecvSingleMessage<'a, T>(&'a SingleMessage<T>);

impl<'a, T> Future for RecvSingleMessage<'a, T> {
    type Output = T;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match self.0.value.take() {
            Some(value) => Poll::Ready(value),
            None => {
                if self.0.waker.replace(Some(cx.waker().clone())).is_some() {
                    panic!("Already recving!");
                }
                Poll::Pending
            }
        }
    }
}

impl<'a, T> std::ops::Drop for RecvSingleMessage<'a, T> {
    fn drop(&mut self) {
        self.0.waker.set(None);
    }
}

#[test]
fn test_abort() {
    let mut long_task_has_completed = [false; 10];
    let mut completing_task_has_completed = false;

    {
        let runtime = nostd_async::Runtime::new();

        let channel = SingleMessage::new();

        let mut blocking_task = Box::new(nostd_async::Task::new(async {
            channel.recv().await;
            panic!("Should not reach here!");
        }));

        let blocking_task_handle = blocking_task.spawn(&runtime);

        let mut long_task = nostd_async::Task::new(async {
            for entry in long_task_has_completed.iter_mut() {
                *entry = true;
                futures_micro::yield_once().await;
            }
        });

        let long_task_handle = long_task.spawn(&runtime);

        let mut completing_task = nostd_async::Task::new(async {
            completing_task_has_completed = true;
        });

        let completing_task_handle = completing_task.spawn(&runtime);

        completing_task_handle.join();

        drop(blocking_task_handle);
        drop(blocking_task);

        channel.send(());

        long_task_handle.join();
    }

    assert!(completing_task_has_completed);
    long_task_has_completed.iter().all(|entry| *entry);
}
