use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

#[repr(i8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RustFuturePoll {
    Ready = 0,
    MaybeReady = 1,
}

pub type RustFutureContinuationCallback = extern "C" fn(callback_data: u64, RustFuturePoll);

#[derive(Debug)]
enum SchedulerState {
    Empty,
    Waked,
    Cancelled,
    Set(RustFutureContinuationCallback, u64),
}

struct Scheduler {
    state: SchedulerState,
}

impl Scheduler {
    fn new() -> Self {
        Self {
            state: SchedulerState::Empty,
        }
    }

    fn store(&mut self, callback: RustFutureContinuationCallback, data: u64) {
        match &self.state {
            SchedulerState::Empty => {
                self.state = SchedulerState::Set(callback, data);
            }
            SchedulerState::Set(old_cb, old_data) => {
                let old_cb = *old_cb;
                let old_data = *old_data;
                old_cb(old_data, RustFuturePoll::Ready);
                self.state = SchedulerState::Set(callback, data);
            }
            SchedulerState::Waked => {
                self.state = SchedulerState::Empty;
                callback(data, RustFuturePoll::MaybeReady);
            }
            SchedulerState::Cancelled => {
                callback(data, RustFuturePoll::Ready);
            }
        }
    }

    fn wake(&mut self) {
        match std::mem::replace(&mut self.state, SchedulerState::Empty) {
            SchedulerState::Set(callback, data) => {
                callback(data, RustFuturePoll::MaybeReady);
            }
            SchedulerState::Empty => {
                self.state = SchedulerState::Waked;
            }
            other => {
                self.state = other;
            }
        }
    }

    fn cancel(&mut self) {
        if let SchedulerState::Set(callback, data) =
            std::mem::replace(&mut self.state, SchedulerState::Cancelled)
        {
            callback(data, RustFuturePoll::Ready);
        }
    }

    fn is_cancelled(&self) -> bool {
        matches!(self.state, SchedulerState::Cancelled)
    }
}

unsafe impl Send for Scheduler {}
unsafe impl Sync for Scheduler {}

enum FutureState<T> {
    Running(Pin<Box<dyn Future<Output = T> + Send + 'static>>),
    Complete(T),
    Taken,
}

pub struct RustFuture<T: Send + 'static> {
    future: Mutex<FutureState<T>>,
    scheduler: Mutex<Scheduler>,
}

impl<T: Send + 'static> RustFuture<T> {
    pub fn new<F>(future: F) -> Arc<Self>
    where
        F: Future<Output = T> + Send + 'static,
    {
        Arc::new(Self {
            future: Mutex::new(FutureState::Running(Box::pin(future))),
            scheduler: Mutex::new(Scheduler::new()),
        })
    }

    pub fn poll(self: &Arc<Self>, callback: RustFutureContinuationCallback, data: u64) {
        let is_cancelled = self.scheduler.lock().unwrap().is_cancelled();

        let is_ready = is_cancelled || {
            let mut future_guard = self.future.lock().unwrap();
            match &mut *future_guard {
                FutureState::Running(fut) => {
                    let waker = self.clone().make_waker();
                    let mut cx = Context::from_waker(&waker);
                    match fut.as_mut().poll(&mut cx) {
                        Poll::Pending => false,
                        Poll::Ready(result) => {
                            *future_guard = FutureState::Complete(result);
                            true
                        }
                    }
                }
                FutureState::Complete(_) => true,
                FutureState::Taken => true,
            }
        };

        if is_ready {
            callback(data, RustFuturePoll::Ready);
        } else {
            self.scheduler.lock().unwrap().store(callback, data);
        }
    }

    pub fn complete(&self) -> Option<T> {
        let mut guard = self.future.lock().unwrap();
        match std::mem::replace(&mut *guard, FutureState::Taken) {
            FutureState::Complete(result) => Some(result),
            other => {
                *guard = other;
                None
            }
        }
    }

    pub fn cancel(&self) {
        self.scheduler.lock().unwrap().cancel();
    }

    pub fn free(self: Arc<Self>) {
        self.scheduler.lock().unwrap().cancel();
    }

    fn make_waker(self: Arc<Self>) -> Waker {
        let raw = RawWaker::new(
            Arc::into_raw(self) as *const (),
            &RawWakerVTable::new(
                Self::waker_clone,
                Self::waker_wake,
                Self::waker_wake_by_ref,
                Self::waker_drop,
            ),
        );
        unsafe { Waker::from_raw(raw) }
    }

    unsafe fn waker_clone(ptr: *const ()) -> RawWaker {
        unsafe { Arc::increment_strong_count(ptr as *const Self) };
        RawWaker::new(
            ptr,
            &RawWakerVTable::new(
                Self::waker_clone,
                Self::waker_wake,
                Self::waker_wake_by_ref,
                Self::waker_drop,
            ),
        )
    }

    unsafe fn waker_wake(ptr: *const ()) {
        let arc = unsafe { Arc::from_raw(ptr as *const Self) };
        arc.scheduler.lock().unwrap().wake();
    }

    unsafe fn waker_wake_by_ref(ptr: *const ()) {
        let ptr = ptr as *const Self;
        unsafe { (*ptr).scheduler.lock().unwrap().wake() };
    }

    unsafe fn waker_drop(ptr: *const ()) {
        drop(unsafe { Arc::from_raw(ptr as *const Self) });
    }
}

pub type RustFutureHandle = *const core::ffi::c_void;

pub fn rust_future_new<F, T>(future: F) -> RustFutureHandle
where
    F: Future<Output = T> + Send + 'static,
    T: Send + 'static,
{
    let arc = RustFuture::new(future);
    Arc::into_raw(arc) as RustFutureHandle
}

pub unsafe fn rust_future_poll<T: Send + 'static>(
    handle: RustFutureHandle,
    callback: RustFutureContinuationCallback,
    data: u64,
) {
    let arc = unsafe { Arc::from_raw(handle as *const RustFuture<T>) };
    arc.poll(callback, data);
    std::mem::forget(arc);
}

pub unsafe fn rust_future_complete<T: Send + 'static>(handle: RustFutureHandle) -> Option<T> {
    let arc = unsafe { Arc::from_raw(handle as *const RustFuture<T>) };
    let result = arc.complete();
    std::mem::forget(arc);
    result
}

pub unsafe fn rust_future_cancel<T: Send + 'static>(handle: RustFutureHandle) {
    let arc = unsafe { Arc::from_raw(handle as *const RustFuture<T>) };
    arc.cancel();
    std::mem::forget(arc);
}

pub unsafe fn rust_future_free<T: Send + 'static>(handle: RustFutureHandle) {
    let arc = unsafe { Arc::from_raw(handle as *const RustFuture<T>) };
    arc.free();
}
