// Test to see what error types are available in nom 8.0.0
use nom::error::*;

fn main() {
    println!("Testing nom error types...");
    
    // Try to use different error types to see what compiles
    let _default_error: Error<&str> = Error::new("test", ErrorKind::Tag);
    println!("Default Error type works");
    
    // Let's see if VerboseError exists under a different name or path
    // This will fail compilation if it doesn't exist
}