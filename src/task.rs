use core::future::Future;
use core::marker::PhantomData;
use core::pin::Pin;
use core::ptr::NonNull;
use core::task::{Context, RawWaker, RawWakerVTable, Waker};

use crate::linked_list::{LinkedListEnds, LinkedListItem, LinkedListLinks};

unsafe fn waker_clone(context: *const ()) -> RawWaker {
    RawWaker::new(context, &RAW_WAKER_VTABLE)
}

unsafe fn waker_wake(context: *const ()) {
    let task = &mut *(context as *mut TaskCore);
    task.insert_back();
}

unsafe fn waker_wake_by_ref(context: *const ()) {
    let task = &mut *(context as *mut TaskCore);
    task.insert_back();
}

unsafe fn waker_drop(_context: *const ()) {}

static RAW_WAKER_VTABLE: RawWakerVTable =
    RawWakerVTable::new(waker_clone, waker_wake, waker_wake_by_ref, waker_drop);

struct TaskCore {
    runtime: NonNull<Runtime>,
    future: Option<*mut dyn Future<Output = ()>>,
    previous: Option<NonNull<TaskCore>>,
    next: Option<NonNull<TaskCore>>,
}

impl TaskCore {
    fn run_once(&mut self) {
        if let Some(future) = self.future {
            self.remove();

            let future = unsafe { Pin::new_unchecked(&mut *future) };
            let data = self as *mut Self as *const ();
            let waker = unsafe { Waker::from_raw(RawWaker::new(data, &RAW_WAKER_VTABLE)) };
            let mut cx = Context::from_waker(&waker);

            if future.poll(&mut cx).is_ready() {
                self.future = None;
            }
        }
    }
}

impl LinkedListItem for TaskCore {
    fn links(
        &self,
    ) -> LinkedListLinks<&Option<NonNull<Self>>, &Option<LinkedListEnds<NonNull<Self>>>> {
        unsafe {
            LinkedListLinks {
                previous: &self.previous,
                next: &self.next,
                ends: &self.runtime.as_ref().tasks,
            }
        }
    }

    fn links_mut(
        &mut self,
    ) -> LinkedListLinks<&mut Option<NonNull<Self>>, &mut Option<LinkedListEnds<NonNull<Self>>>>
    {
        unsafe {
            LinkedListLinks {
                previous: &mut self.previous,
                next: &mut self.next,
                ends: &mut self.runtime.as_mut().tasks,
            }
        }
    }
}

/// A joinable handle for a task.
///
/// The task is aborted if the handle is dropped.
pub struct JoinHandle<'t, T> {
    task_core: &'t mut TaskCore,
    result: &'t mut Option<T>,
}

impl<'t, T> JoinHandle<'t, T> {
    /// Drive the runtime until the handle's task completes.
    pub fn join(self) -> T {
        while self.task_core.future.is_some() {
            unsafe {
                self.task_core.runtime.as_mut().run_once();
            }
        }
        self.result.take().expect("No Result")
    }
}

impl<'t, T> core::ops::Drop for JoinHandle<'t, T> {
    fn drop(&mut self) {
        self.task_core.remove();
    }
}

struct CapturingFuture<F: Future> {
    future: F,
    result: Option<F::Output>,
}

impl<F: Future> Future for CapturingFuture<F> {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> core::task::Poll<Self::Output> {
        let this = unsafe { self.get_unchecked_mut() };
        let future = unsafe { Pin::new_unchecked(&mut this.future) };
        let result = &mut this.result;
        future.poll(cx).map(|value| {
            *result = Some(value);
        })
    }
}

/// An asyncronous task
pub struct Task<'t, F: Future<Output = T> + 't, T: 't> {
    core: Option<TaskCore>,
    future: CapturingFuture<F>,
    _phantom: PhantomData<&'t T>,
}

impl<'t, F: Future<Output = T> + 't, T: 't> Task<'t, F, T> {
    /// Create a new task from a future
    pub fn new(future: F) -> Self {
        Self {
            core: None,
            future: CapturingFuture {
                future,
                result: None,
            },
            _phantom: PhantomData,
        }
    }

    /// Spawn the task into the given runtime.
    /// Note that the task will not be run until a join handle is joined.
    pub fn spawn(&'t mut self, runtime: &'t Runtime) -> JoinHandle<'t, T> {
        if self.core.is_some() {
            panic!("Task already spawned");
        }
        let future =
            unsafe { core::mem::transmute(&mut self.future as *mut dyn Future<Output = ()>) };

        self.core = Some(TaskCore {
            future,
            runtime: NonNull::from(runtime),
            previous: None,
            next: None,
        });

        let task_core = self.core.as_mut().unwrap();

        task_core.insert_back();

        JoinHandle {
            task_core,
            result: &mut self.future.result,
        }
    }
}

/// The asyncronous runtime.
///
/// Note that it is **not threadsafe** and should thus only be run from a single thread.
#[derive(Default)]
pub struct Runtime {
    tasks: Option<LinkedListEnds<NonNull<TaskCore>>>,
}

impl Runtime {
    // Create a new runtime
    pub fn new() -> Self {
        Self::default()
    }

    fn run_once(&mut self) {
        if let Some(tasks) = &mut self.tasks {
            unsafe {
                tasks.first.as_mut().run_once();
            }
        }
    }
}
