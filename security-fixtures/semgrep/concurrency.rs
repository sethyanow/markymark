async fn do_work() {}

async fn bad_async_lock(m: tokio::sync::Mutex<i32>) {
    // ruleid: markymark.rust.await-holding-async-lock
    let guard = m.lock().await;
    do_work().await;
    drop(guard);
}

async fn good_async_lock(m: tokio::sync::Mutex<i32>) {
    let guard = m.lock().await;
    drop(guard);
    // ok: markymark.rust.await-holding-async-lock
    do_work().await;
}

async fn bad_sync_lock(m: std::sync::Mutex<i32>) {
    // ruleid: markymark.rust.await-holding-sync-lock
    let guard = m.lock().unwrap();
    do_work().await;
    drop(guard);
}

async fn good_sync_lock(m: std::sync::Mutex<i32>) {
    let guard = m.lock().unwrap();
    drop(guard);
    // ok: markymark.rust.await-holding-sync-lock
    do_work().await;
}
