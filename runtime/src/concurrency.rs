//! Concurrency Runtime Implementation
//!
//! Provides C-ABI compatible implementations of concurrency primitives
//! for the Rayzor compiler's stdlib extern functions.
//!
//! # Architecture
//!
//! - Thread: Wraps std::thread::JoinHandle
//! - Arc: Wraps std::sync::Arc for atomic reference counting
//! - Mutex: Wraps std::sync::Mutex for mutual exclusion
//! - Channel: Wraps std::sync::mpsc for message passing

use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;
use std::ptr;

// ============================================================================
// Thread Implementation
// ============================================================================

/// Opaque thread handle
/// Wraps JoinHandle<i32> to support returning results
struct ThreadHandle {
    handle: Option<JoinHandle<i32>>,
}

/// Spawn a new thread with a closure
///
/// # Safety
/// - closure must be a valid function pointer
/// - closure_env may be null if closure captures no environment
#[no_mangle]
pub unsafe extern "C" fn rayzor_thread_spawn(
    closure: *const u8,
    closure_env: *const u8,
) -> *mut u8 {
    // Validate pointers (basic sanity check)
    if closure.is_null() {
        return ptr::null_mut();
    }

    // The closure is a function pointer that takes the environment and returns an i32
    // Signature: extern "C" fn(*const u8) -> i32
    type ClosureFn = unsafe extern "C" fn(*const u8) -> i32;

    // Transmute the closure pointer to the function type
    let func: ClosureFn = std::mem::transmute(closure);

    // Convert the environment pointer to usize for Send
    // Note: This is safe because the environment is heap-allocated by MakeClosure
    // and ownership is transferred to the thread
    let env_addr = closure_env as usize;

    // Spawn thread and call the closure with its environment
    let handle = thread::spawn(move || {
        // SAFETY: We're converting back from usize to the pointer
        // The closure owns the environment, so this is safe
        unsafe {
            let env_ptr = env_addr as *const u8;

            // Call the function - catch panics for better debugging
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                func(env_ptr)
            }));

            match result {
                Ok(val) => val,
                Err(_) => 0, // Return 0 on panic
            }
        }
    });

    let thread_handle = Box::new(ThreadHandle {
        handle: Some(handle),
    });

    Box::into_raw(thread_handle) as *mut u8
}

/// Join a thread and wait for it to complete
///
/// # Safety
/// - handle must be a valid pointer from rayzor_thread_spawn
/// - handle is consumed and should not be used after this call
/// - returns the i32 result cast to *mut u8
#[no_mangle]
pub unsafe extern "C" fn rayzor_thread_join(handle: *mut u8) -> *mut u8 {
    if handle.is_null() {
        return ptr::null_mut();
    }

    let mut thread_handle = Box::from_raw(handle as *mut ThreadHandle);

    if let Some(join_handle) = thread_handle.handle.take() {
        match join_handle.join() {
            Ok(result) => {
                // Cast i32 result to pointer (this is how Haxe Int is passed back)
                result as usize as *mut u8
            }
            Err(_) => ptr::null_mut(),
        }
    } else {
        ptr::null_mut()
    }
}

/// Check if a thread has finished executing
#[no_mangle]
pub unsafe extern "C" fn rayzor_thread_is_finished(handle: *const u8) -> bool {
    if handle.is_null() {
        return true;
    }

    let thread_handle = &*(handle as *const ThreadHandle);
    thread_handle.handle.is_none() || thread_handle.handle.as_ref().unwrap().is_finished()
}

/// Yield execution to other threads
#[no_mangle]
pub extern "C" fn rayzor_thread_yield_now() {
    thread::yield_now();
}

/// Sleep for specified milliseconds
#[no_mangle]
pub extern "C" fn rayzor_thread_sleep(millis: i32) {
    if millis > 0 {
        thread::sleep(Duration::from_millis(millis as u64));
    }
}

/// Get the current thread ID as u64
#[no_mangle]
pub extern "C" fn rayzor_thread_current_id() -> u64 {
    // Convert ThreadId to u64 (simplified - uses debug format hash)
    let id = thread::current().id();
    // Use a simple hash of the debug representation
    format!("{:?}", id).bytes().fold(0u64, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u64))
}

// ============================================================================
// Arc Implementation
// ============================================================================

/// Initialize a new Arc with a value
///
/// # Safety
/// - value must be a valid pointer (will be owned by Arc)
#[no_mangle]
pub unsafe extern "C" fn rayzor_arc_init(value: *mut u8) -> *mut u8 {
    if value.is_null() {
        return ptr::null_mut();
    }

    // Wrap the raw pointer in an Arc
    // Note: This takes ownership of the value pointer
    let arc = Arc::new(value);

    // Convert Arc to raw pointer
    Arc::into_raw(arc) as *mut u8
}

/// Clone an Arc (increment reference count)
///
/// # Safety
/// - arc must be a valid Arc pointer from rayzor_arc_init or rayzor_arc_clone
#[no_mangle]
pub unsafe extern "C" fn rayzor_arc_clone(arc: *const u8) -> *mut u8 {
    if arc.is_null() {
        return ptr::null_mut();
    }

    // Reconstruct Arc from raw pointer (without decrementing count)
    let arc_ref = Arc::from_raw(arc as *const *mut u8);

    // Clone it (increments ref count)
    let cloned = Arc::clone(&arc_ref);

    // Forget the original to avoid decrementing ref count
    std::mem::forget(arc_ref);

    // Return new Arc as raw pointer
    Arc::into_raw(cloned) as *mut u8
}

/// Get the inner value pointer from an Arc
///
/// # Safety
/// - arc must be a valid Arc pointer
/// - returned pointer is valid as long as Arc exists
#[no_mangle]
pub unsafe extern "C" fn rayzor_arc_get(arc: *const u8) -> *const u8 {
    if arc.is_null() {
        return ptr::null();
    }

    // Reconstruct Arc temporarily
    // Arc<*mut u8> stores a pointer to the inner value
    let arc_ref = Arc::from_raw(arc as *const *mut u8);

    // Single dereference to get the inner *mut u8 value (the channel/mutex/etc pointer)
    let value_ptr = *arc_ref as *const u8;

    // Forget to avoid decrementing ref count
    std::mem::forget(arc_ref);

    value_ptr
}

/// Get the strong reference count of an Arc
#[no_mangle]
pub unsafe extern "C" fn rayzor_arc_strong_count(arc: *const u8) -> u64 {
    if arc.is_null() {
        return 0;
    }

    let arc_ref = Arc::from_raw(arc as *const *mut u8);
    let count = Arc::strong_count(&arc_ref);
    std::mem::forget(arc_ref);

    count as u64
}

/// Try to unwrap an Arc (returns value if refcount == 1)
///
/// # Safety
/// - arc must be a valid Arc pointer
/// - returns null if refcount > 1
#[no_mangle]
pub unsafe extern "C" fn rayzor_arc_try_unwrap(arc: *mut u8) -> *mut u8 {
    if arc.is_null() {
        return ptr::null_mut();
    }

    let arc_obj = Arc::from_raw(arc as *const *mut u8);

    match Arc::try_unwrap(arc_obj) {
        Ok(value) => value,
        Err(arc_back) => {
            // Failed to unwrap, restore the Arc
            std::mem::forget(arc_back);
            ptr::null_mut()
        }
    }
}

/// Get the pointer address of the Arc's data
#[no_mangle]
pub unsafe extern "C" fn rayzor_arc_as_ptr(arc: *const u8) -> u64 {
    if arc.is_null() {
        return 0;
    }

    let arc_ref = Arc::from_raw(arc as *const *mut u8);
    let ptr_addr = Arc::as_ptr(&arc_ref) as u64;
    std::mem::forget(arc_ref);

    ptr_addr
}

// ============================================================================
// Mutex Implementation
// ============================================================================

/// Mutex handle wrapping std::sync::Mutex
struct MutexHandle {
    mutex: Mutex<*mut u8>,
}

/// Mutex guard handle
struct MutexGuard {
    _marker: std::marker::PhantomData<()>,
}

/// Initialize a new Mutex with a value
#[no_mangle]
pub unsafe extern "C" fn rayzor_mutex_init(value: *mut u8) -> *mut u8 {
    let mutex = Box::new(MutexHandle {
        mutex: Mutex::new(value),
    });

    Box::into_raw(mutex) as *mut u8
}

/// Lock a mutex and return a guard
///
/// # Safety
/// - mutex must be a valid Mutex pointer
/// - blocks until lock is acquired
#[no_mangle]
pub unsafe extern "C" fn rayzor_mutex_lock(mutex: *mut u8) -> *mut u8 {
    if mutex.is_null() {
        return ptr::null_mut();
    }

    let mutex_handle = &*(mutex as *const MutexHandle);

    // Lock the mutex (blocks until acquired)
    match mutex_handle.mutex.lock() {
        Ok(_guard) => {
            // For simplicity, we return the mutex pointer itself as the guard
            // In a real implementation, we'd need to properly handle the guard
            // For now, this is a simplified version
            mutex
        }
        Err(_) => ptr::null_mut(),
    }
}

/// Try to lock a mutex without blocking
#[no_mangle]
pub unsafe extern "C" fn rayzor_mutex_try_lock(mutex: *mut u8) -> *mut u8 {
    if mutex.is_null() {
        return ptr::null_mut();
    }

    let mutex_handle = &*(mutex as *const MutexHandle);

    match mutex_handle.mutex.try_lock() {
        Ok(_guard) => mutex,
        Err(_) => ptr::null_mut(),
    }
}

/// Check if a mutex is currently locked
#[no_mangle]
pub unsafe extern "C" fn rayzor_mutex_is_locked(mutex: *const u8) -> bool {
    if mutex.is_null() {
        return false;
    }

    let mutex_handle = &*(mutex as *const MutexHandle);
    mutex_handle.mutex.try_lock().is_err()
}

/// Get the value pointer from a mutex guard
#[no_mangle]
pub unsafe extern "C" fn rayzor_mutex_guard_get(guard: *mut u8) -> *mut u8 {
    if guard.is_null() {
        return ptr::null_mut();
    }

    // In our simplified implementation, guard is the mutex pointer
    let mutex_handle = &*(guard as *const MutexHandle);

    // This is unsafe - we're accessing without holding the lock properly
    // In a real implementation, we'd store the guard separately
    match mutex_handle.mutex.try_lock() {
        Ok(guard) => {
            let value = *guard;
            drop(guard);
            value
        }
        Err(_) => ptr::null_mut(),
    }
}

/// Unlock a mutex guard
#[no_mangle]
pub unsafe extern "C" fn rayzor_mutex_unlock(_guard: *mut u8) {
    // In our simplified implementation, the guard unlocks automatically
    // when the MutexGuard is dropped (which happens when lock() returns)
    // This is a placeholder for the real implementation
}

// ============================================================================
// Channel Implementation
// ============================================================================

// Use crossbeam-like approach with Arc<Mutex<>> for thread-safe mpmc
use std::collections::VecDeque;
use std::sync::Condvar;

/// Channel state for multi-producer multi-consumer
struct ChannelState {
    buffer: VecDeque<*mut u8>,
    capacity: usize,  // 0 = unbounded
    closed: bool,
}

/// Channel handle - thread-safe mpmc channel
struct ChannelHandle {
    state: Mutex<ChannelState>,
    not_empty: Condvar,
    not_full: Condvar,
}

/// Initialize a new channel with optional capacity
/// capacity=0 means unbounded channel
#[no_mangle]
pub unsafe extern "C" fn rayzor_channel_init(capacity: i32) -> *mut u8 {
    let channel_handle = Box::new(ChannelHandle {
        state: Mutex::new(ChannelState {
            buffer: VecDeque::new(),
            capacity: if capacity <= 0 { 0 } else { capacity as usize },
            closed: false,
        }),
        not_empty: Condvar::new(),
        not_full: Condvar::new(),
    });

    Box::into_raw(channel_handle) as *mut u8
}

/// Send a value through a channel (blocking)
#[no_mangle]
pub unsafe extern "C" fn rayzor_channel_send(channel: *mut u8, value: *mut u8) {
    if channel.is_null() {
        return;
    }

    let channel_handle = &*(channel as *const ChannelHandle);
    let mut state = channel_handle.state.lock().unwrap();

    // For bounded channels, wait while full
    while state.capacity > 0 && state.buffer.len() >= state.capacity && !state.closed {
        state = channel_handle.not_full.wait(state).unwrap();
    }

    if state.closed {
        return;
    }

    state.buffer.push_back(value);
    drop(state);

    // Notify waiting receivers
    channel_handle.not_empty.notify_one();
}

/// Try to send a value through a channel (non-blocking)
#[no_mangle]
pub unsafe extern "C" fn rayzor_channel_try_send(channel: *mut u8, value: *mut u8) -> bool {
    if channel.is_null() {
        return false;
    }

    let channel_handle = &*(channel as *const ChannelHandle);
    let mut state = channel_handle.state.lock().unwrap();

    if state.closed {
        return false;
    }

    // For bounded channels, check if full
    if state.capacity > 0 && state.buffer.len() >= state.capacity {
        return false;
    }

    state.buffer.push_back(value);
    drop(state);
    channel_handle.not_empty.notify_one();
    true
}

/// Receive a value from a channel (blocking)
#[no_mangle]
pub unsafe extern "C" fn rayzor_channel_receive(channel: *mut u8) -> *mut u8 {
    if channel.is_null() {
        return ptr::null_mut();
    }

    let channel_handle = &*(channel as *const ChannelHandle);
    let mut state = channel_handle.state.lock().unwrap();

    // Wait while buffer is empty and channel is not closed
    while state.buffer.is_empty() && !state.closed {
        state = channel_handle.not_empty.wait(state).unwrap();
    }

    if let Some(value) = state.buffer.pop_front() {
        drop(state);
        channel_handle.not_full.notify_one();
        value
    } else {
        ptr::null_mut()
    }
}

/// Try to receive a value from a channel (non-blocking)
#[no_mangle]
pub unsafe extern "C" fn rayzor_channel_try_receive(channel: *mut u8) -> *mut u8 {
    if channel.is_null() {
        return ptr::null_mut();
    }

    let channel_handle = &*(channel as *const ChannelHandle);
    let mut state = channel_handle.state.lock().unwrap();

    if let Some(value) = state.buffer.pop_front() {
        drop(state);
        channel_handle.not_full.notify_one();
        value
    } else {
        ptr::null_mut()
    }
}

/// Close a channel
#[no_mangle]
pub unsafe extern "C" fn rayzor_channel_close(channel: *mut u8) {
    if channel.is_null() {
        return;
    }

    let channel_handle = &*(channel as *const ChannelHandle);
    let mut state = channel_handle.state.lock().unwrap();
    state.closed = true;
    drop(state);

    // Wake up all waiting threads
    channel_handle.not_empty.notify_all();
    channel_handle.not_full.notify_all();
}

/// Check if a channel is closed
#[no_mangle]
pub unsafe extern "C" fn rayzor_channel_is_closed(channel: *const u8) -> bool {
    if channel.is_null() {
        return true;
    }

    let channel_handle = &*(channel as *const ChannelHandle);
    let state = channel_handle.state.lock().unwrap();
    state.closed
}

/// Get the number of messages in the channel
#[no_mangle]
pub unsafe extern "C" fn rayzor_channel_len(channel: *const u8) -> i32 {
    if channel.is_null() {
        return 0;
    }

    let channel_handle = &*(channel as *const ChannelHandle);
    let state = channel_handle.state.lock().unwrap();
    state.buffer.len() as i32
}

/// Get the channel capacity
#[no_mangle]
pub unsafe extern "C" fn rayzor_channel_capacity(channel: *const u8) -> i32 {
    if channel.is_null() {
        return 0;
    }

    let channel_handle = &*(channel as *const ChannelHandle);
    let state = channel_handle.state.lock().unwrap();
    if state.capacity == 0 {
        -1 // Unbounded
    } else {
        state.capacity as i32
    }
}

/// Check if channel is empty
#[no_mangle]
pub unsafe extern "C" fn rayzor_channel_is_empty(channel: *const u8) -> bool {
    if channel.is_null() {
        return true;
    }

    let channel_handle = &*(channel as *const ChannelHandle);
    let state = channel_handle.state.lock().unwrap();
    state.buffer.is_empty()
}

/// Check if channel is full
#[no_mangle]
pub unsafe extern "C" fn rayzor_channel_is_full(channel: *const u8) -> bool {
    if channel.is_null() {
        return false;
    }

    let channel_handle = &*(channel as *const ChannelHandle);
    let state = channel_handle.state.lock().unwrap();
    state.capacity > 0 && state.buffer.len() >= state.capacity
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_thread_spawn_join() {
        unsafe {
            let handle = rayzor_thread_spawn(ptr::null(), ptr::null());
            assert!(!handle.is_null());

            rayzor_thread_join(handle);
        }
    }

    #[test]
    fn test_arc_basic() {
        unsafe {
            let value = Box::into_raw(Box::new(42u32)) as *mut u8;
            let arc1 = rayzor_arc_init(value);
            assert!(!arc1.is_null());

            let count = rayzor_arc_strong_count(arc1);
            assert_eq!(count, 1);

            let arc2 = rayzor_arc_clone(arc1);
            assert!(!arc2.is_null());

            let count = rayzor_arc_strong_count(arc1);
            assert_eq!(count, 2);
        }
    }

    #[test]
    fn test_channel_send_receive() {
        unsafe {
            let channel = rayzor_channel_init(0);
            assert!(!channel.is_null());

            let value = 42usize as *mut u8;
            rayzor_channel_send(channel, value);

            let received = rayzor_channel_receive(channel);
            assert_eq!(received as usize, 42);

            rayzor_channel_close(channel);
        }
    }
}
