//! Test that Math runtime functions work correctly

extern "C" {
    fn haxe_math_abs(x: f64) -> f64;
    fn haxe_math_sqrt(x: f64) -> f64;
    fn haxe_math_min(a: f64, b: f64) -> f64;
    fn haxe_math_max(a: f64, b: f64) -> f64;
    fn haxe_math_sin(x: f64) -> f64;
    fn haxe_math_cos(x: f64) -> f64;
    fn haxe_math_floor(x: f64) -> i32;
    fn haxe_math_ceil(x: f64) -> i32;
    fn haxe_math_round(x: f64) -> i32;
    fn haxe_math_pow(x: f64, y: f64) -> f64;
    fn haxe_math_pi() -> f64;
    fn haxe_math_random() -> f64;
}

fn main() {
    println!("=== Testing Haxe Math Runtime Functions ===\n");

    unsafe {
        // Test constants
        println!("Constants:");
        let pi = haxe_math_pi();
        println!("  Math.PI = {}", pi);
        assert!(
            (pi - std::f64::consts::PI).abs() < 0.0001,
            "PI value incorrect"
        );
        println!("  ✅ Math.PI correct\n");

        // Test abs
        println!("Math.abs:");
        let abs_neg = haxe_math_abs(-5.0);
        let abs_pos = haxe_math_abs(5.0);
        println!("  Math.abs(-5.0) = {}", abs_neg);
        println!("  Math.abs(5.0) = {}", abs_pos);
        assert!((abs_neg - 5.0).abs() < 0.0001, "abs(-5.0) incorrect");
        assert!((abs_pos - 5.0).abs() < 0.0001, "abs(5.0) incorrect");
        println!("  ✅ Math.abs works\n");

        // Test sqrt
        println!("Math.sqrt:");
        let sqrt_16 = haxe_math_sqrt(16.0);
        let sqrt_25 = haxe_math_sqrt(25.0);
        println!("  Math.sqrt(16.0) = {}", sqrt_16);
        println!("  Math.sqrt(25.0) = {}", sqrt_25);
        assert!((sqrt_16 - 4.0).abs() < 0.0001, "sqrt(16.0) incorrect");
        assert!((sqrt_25 - 5.0).abs() < 0.0001, "sqrt(25.0) incorrect");
        println!("  ✅ Math.sqrt works\n");

        // Test min/max
        println!("Math.min/max:");
        let min_val = haxe_math_min(3.0, 7.0);
        let max_val = haxe_math_max(3.0, 7.0);
        println!("  Math.min(3.0, 7.0) = {}", min_val);
        println!("  Math.max(3.0, 7.0) = {}", max_val);
        assert!((min_val - 3.0).abs() < 0.0001, "min incorrect");
        assert!((max_val - 7.0).abs() < 0.0001, "max incorrect");
        println!("  ✅ Math.min/max work\n");

        // Test trigonometry
        println!("Trigonometry:");
        let sin_0 = haxe_math_sin(0.0);
        let cos_0 = haxe_math_cos(0.0);
        let sin_pi_2 = haxe_math_sin(pi / 2.0);
        println!("  Math.sin(0.0) = {}", sin_0);
        println!("  Math.cos(0.0) = {}", cos_0);
        println!("  Math.sin(PI/2) = {}", sin_pi_2);
        assert!(sin_0.abs() < 0.0001, "sin(0) incorrect");
        assert!((cos_0 - 1.0).abs() < 0.0001, "cos(0) incorrect");
        assert!((sin_pi_2 - 1.0).abs() < 0.0001, "sin(PI/2) incorrect");
        println!("  ✅ Trigonometry works\n");

        // Test rounding
        println!("Rounding:");
        let floor_val = haxe_math_floor(3.7);
        let ceil_val = haxe_math_ceil(3.2);
        let round_val = haxe_math_round(3.5);
        println!("  Math.floor(3.7) = {}", floor_val);
        println!("  Math.ceil(3.2) = {}", ceil_val);
        println!("  Math.round(3.5) = {}", round_val);
        assert_eq!(floor_val, 3, "floor incorrect");
        assert_eq!(ceil_val, 4, "ceil incorrect");
        assert_eq!(round_val, 4, "round incorrect");
        println!("  ✅ Rounding works\n");

        // Test pow
        println!("Math.pow:");
        let pow_2_8 = haxe_math_pow(2.0, 8.0);
        let pow_3_3 = haxe_math_pow(3.0, 3.0);
        println!("  Math.pow(2.0, 8.0) = {}", pow_2_8);
        println!("  Math.pow(3.0, 3.0) = {}", pow_3_3);
        assert!((pow_2_8 - 256.0).abs() < 0.0001, "pow(2, 8) incorrect");
        assert!((pow_3_3 - 27.0).abs() < 0.0001, "pow(3, 3) incorrect");
        println!("  ✅ Math.pow works\n");

        // Test random
        println!("Math.random:");
        let rand1 = haxe_math_random();
        let rand2 = haxe_math_random();
        println!("  Math.random() = {}", rand1);
        println!("  Math.random() = {}", rand2);
        assert!((0.0..=1.0).contains(&rand1), "random out of range");
        assert!((0.0..=1.0).contains(&rand2), "random out of range");
        assert!(rand1 != rand2, "random values should differ");
        println!("  ✅ Math.random works\n");
    }

    println!("{}", "=".repeat(70));
    println!("🎉 All Math runtime functions work correctly!");
}
