pub fn main() {
    let runtime = nostd_async::Runtime::new();

    let mut t1 = nostd_async::Task::new(async {
        println!("Hello from Task 1");
        1
    });
    let mut t2 = nostd_async::Task::new(async {
        println!("Hello from Task 2");
        2
    });

    let h1 = t1.spawn(&runtime);
    let h2 = t2.spawn(&runtime);

    // Note that despice the fact that h2 is joined first, h1 runs first
    println!("Task 2: {}", h2.join());
    println!("Task 1: {}", h1.join());
}
