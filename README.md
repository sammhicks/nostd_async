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
