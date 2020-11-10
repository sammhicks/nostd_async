#![no_std]

//!
//! # Example
//! ```
//! let runtime = nostd_async::Runtime::new();
//!
//! let mut task = nostd_async::Task::new(async { println!("Hello World") });
//!
//! let handle = task.spawn(&runtime);
//!
//! handle.join();
//! ```
//! See more examples in the [examples directory]("https://github.com/sammhicks/nostd_async/tree/master/examples")

mod linked_list;
mod task;

pub use task::{JoinHandle, Runtime, Task};
