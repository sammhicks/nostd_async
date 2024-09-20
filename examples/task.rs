pub fn main() {
    let runtime = nostd_async::Runtime::new();

    let task = core::pin::pin!(nostd_async::Task::new(async {
        println!("Hello World");
        42
    }));

    let handle = runtime.spawn(task);

    println!("{}", handle.join());
}
