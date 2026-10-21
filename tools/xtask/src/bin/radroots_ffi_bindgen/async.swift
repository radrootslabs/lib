// radroots-swift-cancellation-v1. Only the observing Rust future is cancelled;
// admitted work and obtained material remain owned by the shared runtime.
// NSLock serializes cancel against release. The unchecked Sendable owner has
// no unsynchronized mutable state; its immutable C callback is Sendable.
fileprivate final class UniffiRustCancellation: @unchecked Sendable {
    private let lock = NSLock()
    private var handle: UInt64?
    private let cancelFunction: @Sendable (UInt64) -> Void
    init(_ handle: UInt64, _ cancelFunction: @escaping @Sendable (UInt64) -> Void) {
        self.handle = handle
        self.cancelFunction = cancelFunction
    }
    func cancel() {
        lock.lock()
        defer { lock.unlock() }
        if let handle { cancelFunction(handle) }
    }
    func finish() {
        lock.lock()
        defer { lock.unlock() }
        handle = nil
    }
}

fileprivate func uniffiRustCallAsync<F, T>(
    rustFutureFunc: () -> UInt64,
    pollFunc: (UInt64, @escaping UniffiRustFutureContinuationCallback, UInt64) -> (),
    completeFunc: (UInt64, UnsafeMutablePointer<RustCallStatus>) -> F,
    cancelFunc: @escaping @Sendable (UInt64) -> Void,
    freeFunc: (UInt64) -> (),
    liftFunc: (F) throws -> T,
    errorHandler: ((RustBuffer) throws -> Swift.Error)?
) async throws -> T {
    try Task.checkCancellation()
    __RADROOTS_ENSURE_INIT__
    let handle = rustFutureFunc()
    let cancellation = UniffiRustCancellation(handle, cancelFunc)
    defer {
        cancellation.finish()
        freeFunc(handle)
    }
    return try await withTaskCancellationHandler {
        var ready: Int8
        repeat {
            ready = await withUnsafeContinuation { continuation in
                let callbackHandle = uniffiContinuationHandleMap.insert(obj: continuation)
                pollFunc(handle, { callbackHandle, status in
                    uniffiFutureContinuationCallback(handle: callbackHandle, pollResult: status)
                }, callbackHandle)
            }
        } while ready != UNIFFI_RUST_FUTURE_POLL_READY
        let result = try makeRustCall({ completeFunc(handle, $0) }, errorHandler: errorHandler)
        return try liftFunc(result)
    } onCancel: {
        cancellation.cancel()
    }
}
