pub fn main() {
    let runtime = nostd_async::Runtime::new();

    let mut t1 = nostd_async::Task::new(async { println!("Task 1") });
    let mut t2 = nostd_async::Task::new(async { println!("Task 2") });

    let h1 = t1.spawn(&runtime);
    let h2 = t2.spawn(&runtime);

    // Note that despice the fact that h2 is joined first, h1 runs first
    h2.join();
    h1.join();
}
