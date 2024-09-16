# nostd_async

**NOTE**: `nostd_async` is no longer under active development, I suggest using [embassy](https://embassy.dev/) instead, which has a much better async runtime.

## Example

```rust
pub fn main() {
    let runtime = nostd_async::Runtime::new();

    let mut task = nostd_async::Task::new(async { println!("Hello World") });

    let handle = task.spawn(&runtime);

    handle.join();
}
```

## Features

### `avr`

Enables AVR Support.

 + Disables interrupts when scheduling and descheduling tasks
 + Waits for interrupts when there are no tasks remaining

### `cortex_m`

Enables Cortex-M Support.

 + Disables interrupts when scheduling and descheduling tasks
 + Waits for events when there are no tasks remaining

