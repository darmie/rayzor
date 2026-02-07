package rayzor.gpu;

/**
 * GPU compute context for accelerated numerical operations.
 *
 * GPUCompute provides GPU-accelerated buffer management and (in future phases)
 * elementwise operations, reductions, and linear algebra via Metal/CUDA/WebGPU.
 *
 * This is an opt-in native package — it requires the rayzor-gpu dynamic library
 * to be available at runtime. Use `GPUCompute.isAvailable()` to check.
 *
 * Example:
 * ```haxe
 * import rayzor.gpu.GPUCompute;
 * import rayzor.ds.Tensor;
 *
 * if (GPUCompute.isAvailable()) {
 *     var gpu = GPUCompute.create();
 *     var t = Tensor.ones([1024], F32);
 *     var buf = gpu.createBuffer(t);
 *     var t2 = gpu.toTensor(buf);
 *     trace(t2.sum());  // 1024.0
 *     gpu.freeBuffer(buf);
 *     gpu.destroy();
 * }
 * ```
 */
@:native("rayzor::gpu::GPUCompute")
extern class GPUCompute {
    /** Create a new GPU compute context. Returns null if GPU is unavailable. */
    @:native("gpu_compute_create")
    public static function create():GPUCompute;

    /** Destroy this GPU compute context and release device resources. */
    @:native("gpu_compute_destroy")
    public function destroy():Void;

    /** Check if GPU compute is available on this system. */
    @:native("gpu_compute_isAvailable")
    public static function isAvailable():Bool;

    /** Create a GPU buffer by copying data from a CPU tensor. */
    @:native("gpu_compute_createBuffer")
    public function createBuffer(tensor:rayzor.ds.Tensor):GpuBuffer;

    /** Allocate an empty GPU buffer with the given element count and dtype. */
    @:native("gpu_compute_allocBuffer")
    public function allocBuffer(numel:Int, dtype:rayzor.ds.DType):GpuBuffer;

    /** Copy GPU buffer data back to a new CPU tensor. */
    @:native("gpu_compute_toTensor")
    public function toTensor(buffer:GpuBuffer):rayzor.ds.Tensor;

    /** Free a GPU buffer. */
    @:native("gpu_compute_freeBuffer")
    public function freeBuffer(buffer:GpuBuffer):Void;
}
