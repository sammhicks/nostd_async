pub fn main() {
    let runtime = nostd_async::Runtime::new();

    let mut task = nostd_async::Task::new(async {
        println!("Hello World");
        42
    });

    let handle = task.spawn(&runtime);

    println!("{}", handle.join());
}
