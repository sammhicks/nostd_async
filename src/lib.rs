#![no_std]

//!
//! # Example
//! ```
//! let runtime = nostd_async::Runtime::new();
//!
//! let mut task = nostd_async::Task::new(async {
//!     println!("Hello World");
//!     42
//! });
//!
//! let handle = task.spawn(&runtime);
//!
//! assert_eq!(handle.join(), 42);
//! ```
//! See more examples in the [examples directory](https://github.com/sammhicks/nostd_async/tree/master/examples)

mod interrupt;
mod linked_list;
pub mod sync;
mod task;

pub use task::{JoinHandle, Runtime, Task};
