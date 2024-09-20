use core::future::Future;

fn task(
    task_index: usize,
    sections_count: usize,
) -> nostd_async::Task<impl Future<Output = usize>> {
    nostd_async::Task::new(async move {
        for section_index in 1..=sections_count {
            println!("Task {} Section {}", task_index, section_index);
            futures_micro::yield_once().await;
        }
        task_index
    })
}

pub fn main() {
    let runtime = nostd_async::Runtime::new();

    let t1 = core::pin::pin!(task(1, 4));
    let t2 = core::pin::pin!(task(2, 4));

    let h1 = runtime.spawn(t1);
    let h2 = runtime.spawn(t2);

    println!("Task 1: {}", h1.join());
    println!("Task 2: {}", h2.join());
}
