package rayzor.gpu;

/**
 * Opaque handle to a GPU-resident buffer.
 *
 * Created via `GPUCompute.createBuffer()` or `GPUCompute.allocBuffer()`.
 * Data can be read back to a CPU tensor via `GPUCompute.toTensor()`.
 *
 * GpuBuffer is an opaque pointer — all operations go through the
 * GPUCompute context that created it.
 */
@:native("rayzor::gpu::GpuBuffer")
extern class GpuBuffer {
    /** Get the number of elements in this buffer. */
    @:native("gpu_buffer_numel")
    public function numel():Int;

    /** Get the dtype tag of this buffer. */
    @:native("gpu_buffer_dtype")
    public function dtype():rayzor.ds.DType;
}
