//! Comprehensive control flow tests for the Haxe parser

use parser::parse_haxe_file;

#[test]
fn test_if_statements() {
    // Simple if
    let input = r#"
class Test {
    function test() {
        if (condition) {
            doSomething();
        }
    }
}
"#;
    
    match parse_haxe_file("test.hx", input, false) {
        Ok(_) => {},
        Err(e) => panic!("Simple if should parse, got: {}", e),
    }
}

#[test]
fn test_if_else_statements() {
    // If-else
    let input = r#"
class Test {
    function test() {
        if (condition) {
            doSomething();
        } else {
            doSomethingElse();
        }
    }
}
"#;
    
    match parse_haxe_file("test.hx", input, false) {
        Ok(_) => {},
        Err(e) => panic!("If-else should parse, got: {}", e),
    }
}

#[test]
fn test_if_else_if_chain() {
    // If-else if chain
    let input = r#"
class Test {
    function test() {
        if (condition1) {
            action1();
        } else if (condition2) {
            action2();
        } else if (condition3) {
            action3();
        } else {
            defaultAction();
        }
    }
}
"#;
    
    match parse_haxe_file("test.hx", input, false) {
        Ok(_) => {},
        Err(e) => panic!("If-else if chain should parse, got: {}", e),
    }
}

#[test]
fn test_nested_if_statements() {
    let input = r#"
class Test {
    function test() {
        if (outer_condition) {
            if (inner_condition) {
                if (deep_condition) {
                    deeply_nested_action();
                }
            } else {
                inner_else_action();
            }
        }
    }
}
"#;
    
    match parse_haxe_file("test.hx", input, false) {
        Ok(_) => {},
        Err(e) => panic!("Nested if statements should parse, got: {}", e),
    }
}

#[test]
fn test_while_loops() {
    let input = r#"
class Test {
    function test() {
        while (condition) {
            doWork();
            updateCondition();
        }
    }
}
"#;
    
    match parse_haxe_file("test.hx", input, false) {
        Ok(_) => {},
        Err(e) => panic!("While loop should parse, got: {}", e),
    }
}

#[test]
fn test_do_while_loops() {
    let input = r#"
class Test {
    function test() {
        do {
            doWork();
            updateCondition();
        } while (condition);
    }
}
"#;
    
    match parse_haxe_file("test.hx", input, false) {
        Ok(_) => {},
        Err(e) => panic!("Do-while loop should parse, got: {}", e),
    }
}

#[test]
fn test_for_loops() {
    let input = r#"
class Test {
    function test() {
        for (item in collection) {
            processItem(item);
        }
    }
}
"#;
    
    match parse_haxe_file("test.hx", input, false) {
        Ok(_) => {},
        Err(e) => panic!("For loop should parse, got: {}", e),
    }
}

#[test]
fn test_nested_loops() {
    let input = r#"
class Test {
    function test() {
        for (row in rows) {
            for (col in columns) {
                while (condition) {
                    processCell(row, col);
                    if (shouldBreak) {
                        break;
                    }
                }
            }
        }
    }
}
"#;
    
    match parse_haxe_file("test.hx", input, false) {
        Ok(_) => {},
        Err(e) => panic!("Nested loops should parse, got: {}", e),
    }
}

#[test]
fn test_switch_statements() {
    let input = r#"
class Test {
    function test() {
        switch (value) {
            case 1:
                handleOne();
            case 2:
                handleTwo();
            case 3 | 4:
                handleThreeOrFour();
            default:
                handleDefault();
        }
    }
}
"#;
    
    match parse_haxe_file("test.hx", input, false) {
        Ok(_) => {},
        Err(e) => panic!("Switch statement should parse, got: {}", e),
    }
}

#[test]
fn test_switch_with_guards() {
    let input = r#"
class Test {
    function test() {
        switch (value) {
            case x if (x > 0):
                handlePositive(x);
            case x if (x < 0):
                handleNegative(x);
            case 0:
                handleZero();
            default:
                handleOther();
        }
    }
}
"#;
    
    match parse_haxe_file("test.hx", input, false) {
        Ok(_) => {},
        Err(e) => panic!("Switch with guards should parse, got: {}", e),
    }
}

#[test]
fn test_try_catch_blocks() {
    let input = r#"
class Test {
    function test() {
        try {
            riskyOperation();
        } catch (e: String) {
            handleStringError(e);
        } catch (e: Dynamic) {
            handleGenericError(e);
        }
    }
}
"#;
    
    match parse_haxe_file("test.hx", input, false) {
        Ok(_) => {},
        Err(e) => panic!("Try-catch should parse, got: {}", e),
    }
}

#[test]
fn test_control_flow_statements() {
    let input = r#"
class Test {
    function test() {
        for (item in items) {
            if (shouldSkip(item)) {
                continue;
            }
            
            if (shouldStop(item)) {
                break;
            }
            
            if (shouldFail(item)) {
                throw "Failed processing item";
            }
            
            processItem(item);
        }
        
        return result;
    }
}
"#;
    
    match parse_haxe_file("test.hx", input, false) {
        Ok(_) => {},
        Err(e) => panic!("Control flow statements should parse, got: {}", e),
    }
}

#[test]
fn test_complex_nested_control_flow() {
    // This is similar to the failing test case
    let input = r#"
class Outer {
    function method() {
        if (condition) {
            for (i in array) {
                switch (value) {
                    case pattern:
                        if (nested_condition) {
                            try {
                                deep_call();
                            } catch (e: Dynamic) {
                                handle_error(e);
                            }
                        }
                    default:
                        continue;
                }
            }
        }
    }
}
"#;
    
    match parse_haxe_file("test.hx", input, false) {
        Ok(_) => {},
        Err(e) => panic!("Complex nested control flow should parse, got: {}", e),
    }
}

#[test]
fn test_expression_vs_statement_contexts() {
    let input = r#"
class Test {
    function test() {
        // If as expression
        var result = if (condition) value1 else value2;
        
        // Switch as expression  
        var type = switch (input) {
            case "string": StringType;
            case "number": NumberType;
            default: UnknownType;
        };
        
        // Try as expression
        var parsed = try {
            parseValue(input);
        } catch (e: Dynamic) {
            defaultValue;
        };
    }
}
"#;
    
    match parse_haxe_file("test.hx", input, false) {
        Ok(_) => {},
        Err(e) => panic!("Expression contexts should parse, got: {}", e),
    }
}