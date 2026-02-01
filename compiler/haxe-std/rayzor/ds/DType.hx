package rayzor.ds;

/**
 * Data type for tensor elements.
 *
 * Determines the numeric precision and storage size of each element
 * in a Tensor. Maps to runtime dtype tags (i64 constants) at MIR level.
 */
enum DType {
    F32;
    F16;
    BF16;
    I32;
    I8;
    U8;
}
