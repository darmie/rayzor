use nom::error;

fn main() {
    // List all available types in nom::error
    println!("Available in nom::error:");
    
    // Try to use different error types
    let _default: error::Error<&str> = error::Error::new("", error::ErrorKind::Tag);
    println!("Error: available");
    
    // Try VerboseError - this will show us if it exists
    // let _verbose: error::VerboseError<&str> = error::VerboseError::new();
}