pub use boltffi_core::{
    CallbackForeignType, CallbackHandle, CustomFfiConvertible, CustomTypeConversionError, Data,
    EventSubscription, FfiType, FromCallbackHandle, StreamProducer, UnexpectedFfiCallbackError,
    custom_ffi, custom_type, data, default, error, export, ffi_stream, name, skip,
};

#[doc(hidden)]
pub mod __private {
    #[cfg(target_arch = "wasm32")]
    pub use boltffi_core::{
        AsyncCallbackCompletion, AsyncCallbackCompletionCode, AsyncCallbackCompletionResult,
        AsyncCallbackRegistry, AsyncCallbackRequestGuard, AsyncCallbackRequestId,
        WasmCallbackOutBuf, WasmCallbackOwner, rust_future_panic_message, rust_future_poll_sync,
        write_return_slot,
    };
    pub use boltffi_core::{
        CallbackForeignType, CallbackHandle, EventSubscription, FfiBuf, FfiSpan, FfiStatus,
        FromCallbackHandle, Passable, RustFutureContinuationCallback, RustFutureHandle, Seal,
        StreamContinuationCallback, StreamPollResult, SubscriptionHandle, VecTransport, WaitResult,
        WirePassable, rustfuture, set_last_error, wire,
    };
}
