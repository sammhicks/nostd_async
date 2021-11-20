use core::{
    future::Future,
    pin::Pin,
    task::{Context, RawWaker, RawWakerVTable, Waker},
};

use bare_metal::CriticalSection;

use crate::{
    cell::Cell,
    interrupt,
    linked_list::{LinkedList, LinkedListItem, LinkedListLinks},
    non_null::NonNull,
};

unsafe fn waker_clone(context: *const ()) -> RawWaker {
    RawWaker::new(context, &RAW_WAKER_VTABLE)
}

unsafe fn waker_wake(context: *const ()) {
    let task = &*(context as *const TaskCore);
    interrupt::free(|cs| task.insert_back(cs));
}

unsafe fn waker_wake_by_ref(context: *const ()) {
    let task = &*(context as *const TaskCore);
    interrupt::free(|cs| task.insert_back(cs));
}

unsafe fn waker_drop(_context: *const ()) {}

static RAW_WAKER_VTABLE: RawWakerVTable =
    RawWakerVTable::new(waker_clone, waker_wake, waker_wake_by_ref, waker_drop);

struct TaskCore {
    runtime: NonNull<Runtime>,
    future: Cell<Option<*mut dyn Future<Output = ()>>>,
    links: LinkedListLinks<Self>,
}

impl TaskCore {
    fn run_once(&self, cs: &CriticalSection) {
        if let Some(future_ptr) = self.future.take() {
            self.remove(cs);

            let future = unsafe { Pin::new_unchecked(&mut *future_ptr) };
            let data = self as *const Self as *const ();
            let waker = unsafe { Waker::from_raw(RawWaker::new(data, &RAW_WAKER_VTABLE)) };
            let mut cx = Context::from_waker(&waker);

            if !future.poll(&mut cx).is_ready() {
                self.future.set(Some(future_ptr));
            }
        }
    }
}

impl LinkedListItem for TaskCore {
    fn links(&self) -> &LinkedListLinks<Self> {
        &self.links
    }

    fn list(&self) -> &LinkedList<Self> {
        unsafe { &self.runtime.as_ref().tasks }
    }
}

/// A joinable handle for a task.
///
/// The task is aborted if the handle is dropped.
pub struct JoinHandle<'a, T> {
    task_core: &'a TaskCore,
    result: &'a mut Option<T>,
}

impl<'a, T> JoinHandle<'a, T> {
    /// Drive the runtime until the handle's task completes.
    ///
    /// Returns the value returned by the future
    pub fn join(self) -> T {
        while self.task_core.future.has_some() {
            unsafe { self.task_core.runtime.as_ref().run_once() };
        }
        self.result.take().expect("No Result")
    }
}

impl<'a, T> Drop for JoinHandle<'a, T> {
    fn drop(&mut self) {
        interrupt::free(|cs| self.task_core.remove(cs));
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
pub struct Task<F: Future> {
    core: Option<TaskCore>,
    future: Option<CapturingFuture<F>>,
}

impl<'a, F> Task<F>
where
    F: Future + 'a,
    F::Output: 'a,
{
    /// Create a new task from a future
    pub fn new(future: F) -> Self {
        Self {
            core: None,
            future: Some(CapturingFuture {
                future,
                result: None,
            }),
        }
    }

    /// Spawn the task into the given runtime.
    /// Note that the task will not be run until a join handle is joined.
    pub fn spawn(&'a mut self, runtime: &'a Runtime) -> JoinHandle<'a, F::Output> {
        if self.core.is_some() {
            panic!("Task already spawned");
        }

        // self.future is only None on drop, so this is safe
        let capturing_future = self.future.as_mut().unwrap();

        let future = Cell::new(Some(unsafe {
            core::mem::transmute::<_, *mut dyn Future<Output = ()>>(
                capturing_future as *mut dyn Future<Output = ()>,
            )
        }));

        self.core = Some(TaskCore {
            future,
            runtime: NonNull::new(runtime),
            links: LinkedListLinks::default(),
        });

        let task_core = self.core.as_ref().unwrap();

        interrupt::free(|cs| task_core.insert_back(cs));

        JoinHandle {
            task_core,
            result: &mut capturing_future.result,
        }
    }
}

impl<F: Future> core::ops::Drop for Task<F> {
    fn drop(&mut self) {
        interrupt::free(|cs| {
            self.future = None;

            if let Some(core) = self.core.take() {
                core.remove(cs);
            }
        })
    }
}

/// The asyncronous runtime.
///
/// Note that it is **not threadsafe** and should thus only be run from a single thread.
#[derive(Default)]
pub struct Runtime {
    tasks: LinkedList<TaskCore>,
}

impl Runtime {
    // Create a new runtime
    pub fn new() -> Self {
        Self::default()
    }

    unsafe fn run_once(&self) {
        interrupt::free(|cs| {
            if self
                .tasks
                .with_first(cs, |first| first.run_once(cs))
                .is_none()
            {
                #[cfg(feature = "cortex_m")]
                {
                    cortex_m::asm::wfi();
                }
            }
        });
    }
}
