package rayzor.core;

/**
	Represents either a success value of type `T` or an error value of type `E`.

	@see https://haxe.org/manual/types-enum-instance.html
**/
enum Result<T, E> {
	Ok(v:T);
	Error(e:E);
}
