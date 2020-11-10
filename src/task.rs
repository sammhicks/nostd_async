use core::future::Future;
use core::marker::PhantomData;
use core::pin::Pin;
use core::ptr::NonNull;
use core::task::{Context, RawWaker, RawWakerVTable, Waker};

use crate::linked_list::{LinkedListEnds, LinkedListItem, LinkedListLinks};

unsafe fn waker_clone(context: *const ()) -> RawWaker {
    RawWaker::new(context, &RAW_WAKER_VTABLE)
}

unsafe fn waker_wake(_context: *const ()) {}

unsafe fn waker_wake_by_ref(_context: *const ()) {}

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
            let waker = unsafe { Waker::from_raw(RawWaker::new(&(), &RAW_WAKER_VTABLE)) };
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

pub struct JoinHandle<'t>(&'t mut TaskCore);

impl<'t> JoinHandle<'t> {
    pub fn join(self) {
        while self.0.future.is_some() {
            unsafe {
                self.0.runtime.as_mut().run_once();
            }
        }
    }
}

impl<'t> core::ops::Drop for JoinHandle<'t> {
    fn drop(&mut self) {
        self.0.remove();
    }
}

pub struct Task<'t, F: Future<Output = ()> + 't> {
    core: Option<TaskCore>,
    future: F,
    _phantom: PhantomData<&'t ()>,
}

impl<'t, F: Future<Output = ()> + 't> Task<'t, F> {
    pub fn new(future: F) -> Self {
        Self {
            core: None,
            future,
            _phantom: PhantomData,
        }
    }

    pub fn spawn(&'t mut self, runtime: &'t Runtime) -> JoinHandle<'t> {
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

        let core = self.core.as_mut().unwrap();

        core.insert_back();

        JoinHandle(core)
    }
}

#[derive(Default)]
pub struct Runtime {
    tasks: Option<LinkedListEnds<NonNull<TaskCore>>>,
}

impl Runtime {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn spawn<'t, F: Future<Output = ()> + 't>(
        &'t self,
        task: &'t mut Task<'t, F>,
    ) -> JoinHandle<'t> {
        task.spawn(self)
    }

    fn run_once(&mut self) {
        if let Some(tasks) = &mut self.tasks {
            unsafe {
                tasks.first.as_mut().run_once();
            }
        }
    }
}
