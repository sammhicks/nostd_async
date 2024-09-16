use core::{
    cell::UnsafeCell,
    future::Future,
    pin::Pin,
    task::{Context, RawWaker, RawWakerVTable, Waker},
};

use crate::{
    linked_list::{LinkedList, LinkedListItem, LinkedListLinks},
    mutex::Mutex,
    non_null::NonNull,
};

unsafe fn waker_clone(context: *const ()) -> RawWaker {
    RawWaker::new(context, &RAW_WAKER_VTABLE)
}

unsafe fn waker_wake(context: *const ()) {
    critical_section::with(|cs| context.cast::<TaskCore>().as_ref().unwrap().insert_back(cs));
}

unsafe fn waker_wake_by_ref(context: *const ()) {
    critical_section::with(|cs| context.cast::<TaskCore>().as_ref().unwrap().insert_back(cs));
}

unsafe fn waker_drop(_context: *const ()) {}

static RAW_WAKER_VTABLE: RawWakerVTable =
    RawWakerVTable::new(waker_clone, waker_wake, waker_wake_by_ref, waker_drop);

trait TaskHandle {
    fn poll_task(&self, cx: &mut Context) -> core::task::Poll<()>;
}

struct TaskCore {
    runtime: NonNull<Runtime>,
    task_handle: Mutex<Option<core::ptr::NonNull<dyn TaskHandle>>>,
    links: LinkedListLinks<Self>,
}

impl TaskCore {
    fn run_once(&self) {
        if let Some(mut task_handle) = critical_section::with(|cs| self.task_handle.take(cs)) {
            let data = core::ptr::from_ref(self).cast();
            let waker = unsafe { Waker::from_raw(RawWaker::new(data, &RAW_WAKER_VTABLE)) };
            let mut cx = Context::from_waker(&waker);

            if unsafe { task_handle.as_mut() }
                .poll_task(&mut cx)
                .is_pending()
            {
                critical_section::with(|cs| self.task_handle.set(cs, Some(task_handle)));
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
    result: &'a Mutex<Option<T>>,
}

impl<'a, T> JoinHandle<'a, T> {
    /// Drive the runtime until the handle's task completes.
    ///
    /// Returns the value returned by the future
    ///
    /// # Panics
    ///
    /// Panics if there's a bug in `nostd_async`
    pub fn join(self) -> T {
        while critical_section::with(|cs| self.task_core.task_handle.has_some(cs)) {
            unsafe { self.task_core.runtime.as_ref().run_once() };
        }

        critical_section::with(|cs| self.result.take(cs).expect("No Result"))
    }
}

impl<'a, T> Drop for JoinHandle<'a, T> {
    fn drop(&mut self) {
        critical_section::with(|cs| self.task_core.remove(cs));
    }
}

struct CapturingFuture<F: Future> {
    future: UnsafeCell<F>,
    result: Mutex<Option<F::Output>>,
}

impl<F: Future> TaskHandle for CapturingFuture<F> {
    fn poll_task(&self, cx: &mut Context<'_>) -> core::task::Poll<()> {
        unsafe { Pin::new_unchecked(&mut *self.future.get()) }
            .poll(cx)
            .map(|output| critical_section::with(|cs| self.result.set(cs, Some(output))))
    }
}

/// An asyncronous task
pub struct Task<F: Future> {
    core: Option<TaskCore>,
    future: CapturingFuture<F>,
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
            future: CapturingFuture {
                future: UnsafeCell::new(future),
                result: Mutex::new(None),
            },
        }
    }

    /// Spawn the task into the given runtime.
    /// Note that the task will not be run until a join handle is joined.
    ///
    /// # Panics
    ///
    /// Panics if the task has already been spawned
    pub fn spawn(&'a mut self, runtime: &'a Runtime) -> JoinHandle<'a, F::Output> {
        assert!(self.core.is_none(), "Task already spawned");

        let future = unsafe {
            Mutex::new(Some(core::ptr::NonNull::from(core::mem::transmute::<
                &mut (dyn TaskHandle + 'a),
                &mut dyn TaskHandle,
            >(
                &mut self.future
            ))))
        };

        let task_core = {
            let task_core = self.core.get_or_insert(TaskCore {
                task_handle: future,
                runtime: NonNull::new(runtime),
                links: LinkedListLinks::default(),
            });

            critical_section::with(move |cs| task_core.insert_back(cs))
        };

        JoinHandle {
            task_core,
            result: &self.future.result,
        }
    }
}

impl<F: Future> core::ops::Drop for Task<F> {
    fn drop(&mut self) {
        critical_section::with(|cs| {
            if let Some(core) = self.core.take() {
                core.remove(cs);
            }
        });
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
        if let Some(first_task) = critical_section::with(|cs| self.tasks.pop_first(cs)) {
            first_task.run_once();
        } else {
            #[cfg(feature = "avr")]
            avr_device::asm::sleep();
            #[cfg(feature = "cortex_m")]
            cortex_m::asm::wfe();
        }
    }
}
