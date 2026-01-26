/**
 * testmath.c - Test HDLL library for rayzor integration testing.
 *
 * This library emulates the Hashlink HDLL pattern where DEFINE_PRIM
 * generates hlp_ symbols that provide type signatures and function pointers.
 *
 * Build:
 *   macOS:  cc -shared -o testmath.hdll testmath.c
 *   Linux:  cc -shared -fPIC -o testmath.hdll testmath.c
 */

#include <stdlib.h>

/* ---- Actual library functions ---- */

int testmath_add(int a, int b) {
    return a + b;
}

int testmath_multiply(int a, int b) {
    return a * b;
}

double testmath_sqrt_approx(double x) {
    /* Newton's method, 10 iterations */
    if (x <= 0.0) return 0.0;
    double guess = x / 2.0;
    for (int i = 0; i < 10; i++) {
        guess = (guess + x / guess) / 2.0;
    }
    return guess;
}

/* ---- hlp_ introspection symbols (emulating DEFINE_PRIM) ---- */

/*
 * Each hlp_<name> function:
 *   - Sets *sign to a type signature string
 *   - Returns the actual function pointer
 *
 * Signature format: param_type_codes + "_" + return_type_code
 *   i = i32, d = f64, v = void, b = bool, B = bytes, etc.
 */

void* hlp_add(const char** sign) {
    *sign = "ii_i";  /* fn(i32, i32) -> i32 */
    return (void*)testmath_add;
}

void* hlp_multiply(const char** sign) {
    *sign = "ii_i";  /* fn(i32, i32) -> i32 */
    return (void*)testmath_multiply;
}

void* hlp_sqrt_approx(const char** sign) {
    *sign = "d_d";  /* fn(f64) -> f64 */
    return (void*)testmath_sqrt_approx;
}
