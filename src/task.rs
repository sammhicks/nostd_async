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

struct TaskCore {
    runtime: NonNull<Runtime>,
    task_handle: Mutex<Option<core::ptr::NonNull<dyn Future<Output = ()>>>>,
    links: LinkedListLinks<Self>,
}

impl TaskCore {
    fn run_once(&self) {
        if let Some(mut task_handle) = critical_section::with(|cs| self.task_handle.take(cs)) {
            let data = (self as *const Self).cast();
            let waker = unsafe { Waker::from_raw(RawWaker::new(data, &RAW_WAKER_VTABLE)) };
            let mut cx = Context::from_waker(&waker);

            if unsafe { Pin::new_unchecked(task_handle.as_mut()) }
                .poll(&mut cx)
                .is_pending()
            {
                critical_section::with(|cs| self.task_handle.set(cs, Some(task_handle)));
            }
        }
    }
}

impl core::ops::Drop for TaskCore {
    fn drop(&mut self) {
        critical_section::with(|cs| self.remove(cs));
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

struct CapturingFuture<F: Future> {
    future: UnsafeCell<F>,
    result: Mutex<Option<F::Output>>,
}

impl<F: Future> Future for CapturingFuture<F> {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> core::task::Poll<Self::Output> {
        unsafe { Pin::new_unchecked(&mut *self.future.get()) }
            .poll(cx)
            .map(|output| critical_section::with(|cs| self.result.set(cs, Some(output))))
    }
}

/// An asyncronous task
#[pin_project::pin_project(project = TaskProjection)]
pub struct Task<F: Future> {
    core: Option<TaskCore>,
    future: CapturingFuture<F>,
    _pinned: core::marker::PhantomPinned,
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
            _pinned: core::marker::PhantomPinned,
        }
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

    /// Spawn the task.
    /// Note that the task will not be run until a join handle is joined.
    ///
    /// # Panics
    ///
    /// Panics if the task has already been spawned
    pub fn spawn<'a, F: Future>(&'a self, task: Pin<&'a mut Task<F>>) -> JoinHandle<'a, F::Output> {
        // Safety
        let TaskProjection {
            core,
            future,
            _pinned: _,
        } = task.project();

        assert!(core.is_none(), "Task already spawned");

        let task_handle = unsafe {
            Mutex::new(Some(core::ptr::NonNull::from(core::mem::transmute::<
                &mut (dyn Future<Output = ()> + 'a),
                &mut dyn Future<Output = ()>,
            >(future))))
        };

        let task_core = {
            let task_core = core.get_or_insert(TaskCore {
                task_handle,
                runtime: NonNull::new(self),
                links: LinkedListLinks::default(),
            });

            critical_section::with(move |cs| task_core.insert_back(cs))
        };

        JoinHandle {
            task_core,
            result: &future.result,
        }
    }

    unsafe fn run_once(&self) {
        let first_task = critical_section::with(|cs| {
            let first_task = self.tasks.pop_first(cs);

            if first_task.is_none() {
                #[cfg(feature = "avr")]
                avr_device::asm::sleep();
                #[cfg(feature = "cortex_m")]
                cortex_m::asm::wfe();
            }

            first_task
        });

        if let Some(first_task) = first_task {
            first_task.run_once();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Runtime, Task};

    #[test]
    fn test_never_spawned() {
        let task = super::Task::new(async { 1 });

        drop(task);
    }

    #[test]
    fn test_never_driven() {
        let runtime = Runtime::new();

        {
            let task = core::pin::pin!(Task::new(async { 1 }));

            runtime.spawn(task);
        }

        {
            let task = core::pin::pin!(Task::new(async { 1 }));

            assert_eq!(runtime.spawn(task).join(), 1);
        }
    }

    #[test]
    fn test_drop_handle() {
        let runtime = Runtime::new();

        let mut t1 = core::pin::pin!(Task::new(async { 1 }));

        let t2 = core::pin::pin!(Task::new(async { 2 }));

        {
            runtime.spawn(t1.as_mut());
        }
        let h2 = runtime.spawn(t2);

        assert_eq!(h2.join(), 2);

        unsafe {
            assert_eq!(
                critical_section::with(|cs| t1.get_unchecked_mut().future.result.take(cs)).unwrap(),
                1
            );
        }
    }
}
