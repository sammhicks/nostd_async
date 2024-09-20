pub fn main() {
    let runtime = nostd_async::Runtime::new();

    let t1 = core::pin::pin!(nostd_async::Task::new(async {
        println!("Hello from Task 1");
        1
    }));

    let t2 = core::pin::pin!(nostd_async::Task::new(async {
        println!("Hello from Task 2");
        2
    }));

    let h1 = runtime.spawn(t1);
    let h2 = runtime.spawn(t2);

    // Note that despice the fact that h2 is joined first, h1 runs first
    println!("Task 2: {}", h2.join());
    println!("Task 1: {}", h1.join());
}
