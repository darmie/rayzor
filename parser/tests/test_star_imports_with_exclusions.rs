use parser::{haxe_ast::*, parse_haxe_file};

#[test]
fn test_star_import_basic() {
    let code = r#"
package com.example;

import haxe.io.*;

class Test {}
"#;

    let result = parse_haxe_file("test.hx", code, false);
    assert!(result.is_ok());

    let file = result.unwrap();
    assert_eq!(file.imports.len(), 1);

    let import = &file.imports[0];
    assert_eq!(import.path, vec!["haxe", "io"]);
    assert_eq!(import.mode, ImportMode::Wildcard);
}

#[test]
fn test_star_import_with_single_exclusion() {
    let code = r#"
package com.example;

import haxe.io.* except Bytes;

class Test {}
"#;

    let result = parse_haxe_file("test.hx", code, false);
    assert!(result.is_ok());

    let file = result.unwrap();
    assert_eq!(file.imports.len(), 1);

    let import = &file.imports[0];
    assert_eq!(import.path, vec!["haxe", "io"]);
    assert_eq!(
        import.mode,
        ImportMode::WildcardWithExclusions(vec!["Bytes".to_string()])
    );
}

#[test]
fn test_star_import_with_multiple_exclusions() {
    let code = r#"
package com.example;

import haxe.io.* except Bytes, Input, Output;

class Test {}
"#;

    let result = parse_haxe_file("test.hx", code, false);
    assert!(result.is_ok());

    let file = result.unwrap();
    assert_eq!(file.imports.len(), 1);

    let import = &file.imports[0];
    assert_eq!(import.path, vec!["haxe", "io"]);
    assert_eq!(
        import.mode,
        ImportMode::WildcardWithExclusions(vec![
            "Bytes".to_string(),
            "Input".to_string(),
            "Output".to_string()
        ])
    );
}

#[test]
fn test_multiple_imports_with_exclusions() {
    let code = r#"
package com.example;

import haxe.io.* except Bytes;
import haxe.macro.*;
import sys.io.* except File, FileInput;

class Test {}
"#;

    let result = parse_haxe_file("test.hx", code, false);
    assert!(result.is_ok());

    let file = result.unwrap();
    assert_eq!(file.imports.len(), 3);

    // First import with exclusion
    assert_eq!(file.imports[0].path, vec!["haxe", "io"]);
    assert_eq!(
        file.imports[0].mode,
        ImportMode::WildcardWithExclusions(vec!["Bytes".to_string()])
    );

    // Second import without exclusion
    assert_eq!(file.imports[1].path, vec!["haxe", "macro"]);
    assert_eq!(file.imports[1].mode, ImportMode::Wildcard);

    // Third import with multiple exclusions
    assert_eq!(file.imports[2].path, vec!["sys", "io"]);
    assert_eq!(
        file.imports[2].mode,
        ImportMode::WildcardWithExclusions(vec!["File".to_string(), "FileInput".to_string()])
    );
}

#[test]
fn test_star_import_with_exclusions_whitespace_handling() {
    let code = r#"
package com.example;

import haxe.io.* except    Bytes ,   Input  ,Output;

class Test {}
"#;

    let result = parse_haxe_file("test.hx", code, false);
    assert!(result.is_ok());

    let file = result.unwrap();
    assert_eq!(file.imports.len(), 1);

    let import = &file.imports[0];
    assert_eq!(import.path, vec!["haxe", "io"]);
    assert_eq!(
        import.mode,
        ImportMode::WildcardWithExclusions(vec![
            "Bytes".to_string(),
            "Input".to_string(),
            "Output".to_string()
        ])
    );
}

#[test]
fn test_star_import_in_import_hx() {
    let code = r#"
import haxe.io.* except Bytes;
import sys.* except FileSystem, File;
using Lambda;
"#;

    let result = parse_haxe_file("import.hx", code, false);
    assert!(result.is_ok());

    let file = result.unwrap();
    assert_eq!(file.imports.len(), 2);
    assert_eq!(file.using.len(), 1);

    // First import
    assert_eq!(file.imports[0].path, vec!["haxe", "io"]);
    assert_eq!(
        file.imports[0].mode,
        ImportMode::WildcardWithExclusions(vec!["Bytes".to_string()])
    );

    // Second import
    assert_eq!(file.imports[1].path, vec!["sys"]);
    assert_eq!(
        file.imports[1].mode,
        ImportMode::WildcardWithExclusions(vec!["FileSystem".to_string(), "File".to_string()])
    );
}
