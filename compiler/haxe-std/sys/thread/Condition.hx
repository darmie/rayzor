package sys.thread;

#if (!target.threaded)
#error "This class is not available on this target"
#end

/**
	Creates a new condition variable.
	Conditions variables can be used to block one or more threads at the same time,
	until another thread modifies a shared variable (the condition)
	and signals the condition variable.

	Backed by Rayzor's native Condvar (condition variable) implementation
	using Rust's std::sync::Condvar with an internal Mutex.
**/
extern class Condition {
	/**
		Create a new condition variable.
		A thread that waits on a newly created condition variable will block.
	**/
	@:native("sys_condition_alloc")
	function new():Void;

	/**
		Acquires the internal mutex.
	**/
	@:native("sys_condition_acquire")
	function acquire():Void;

	/**
		Tries to acquire the internal mutex.
		@see `Mutex.tryAcquire`
	**/
	@:native("sys_condition_try_acquire")
	function tryAcquire():Bool;

	/***
		Releases the internal mutex.
	**/
	@:native("sys_condition_release")
	function release():Void;

	/**
		Atomically releases the mutex and blocks until the condition variable pointed is signaled by a call to
		`signal` or to `broadcast`. When the calling thread becomes unblocked it
		acquires the internal mutex.
		The internal mutex should be locked before this function is called.
	**/
	@:native("sys_condition_wait")
	function wait():Void;

	/**
		Unblocks one of the threads that are blocked on the
		condition variable at the time of the call. If no threads are blocked
		on the condition variable at the time of the call, the function does nothing.
	**/
	@:native("sys_condition_signal")
	function signal():Void;

	/**
		Unblocks all of the threads that are blocked on the
		condition variable at the time of the call. If no threads are blocked
		on the condition variable at the time of the call, the function does
		nothing.
	**/
	@:native("sys_condition_broadcast")
	function broadcast():Void;
}
