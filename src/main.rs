use snake_online::GameServer;
use std::future::Future;
use std::task::{Context, Poll, Waker};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Executor simples para rodar futures
    let mut server = GameServer::new();
    block_on(server.run("127.0.0.1:8080"))
}

// Executor simples que bloqueia até o future completar
fn block_on<T>(future: impl Future<Output = T>) -> T {
    let mut future = Box::pin(future);
    let waker = create_waker();
    let mut cx = Context::from_waker(&waker);
    
    loop {
        match future.as_mut().poll(&mut cx) {
            Poll::Ready(value) => return value,
            Poll::Pending => {
                // Simples yield - em um executor real, aqui esperaríamos por I/O
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
        }
    }
}

fn create_waker() -> Waker {
    use std::task::{RawWaker, RawWakerVTable};
    
    fn clone_waker(_: *const ()) -> RawWaker {
        RawWaker::new(std::ptr::null(), &VTABLE)
    }
    
    fn wake(_: *const ()) {}
    fn wake_by_ref(_: *const ()) {}
    fn drop_waker(_: *const ()) {}
    
    const VTABLE: RawWakerVTable = RawWakerVTable::new(clone_waker, wake, wake_by_ref, drop_waker);
    
    unsafe {
        Waker::from_raw(RawWaker::new(std::ptr::null(), &VTABLE))
    }
}
