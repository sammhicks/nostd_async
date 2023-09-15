# nostd_async

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
 + Waits for interrupts when there are no tasks remaining

### `wfe` (also enables `cortex_m`)

Uses `wfe` instead of `wfi` if there are no pending tasks
