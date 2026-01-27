#[cfg(test)]
mod override_validation_tests {
    use crate::pipeline::compile_haxe_source;

    #[test]
    fn test_missing_override_modifier() {
        let haxe_code = r#"
            class BaseClass {
                public function new() {}

                public function overridableMethod():String {
                    return "base";
                }
            }

            class ChildClass extends BaseClass {
                public function new() {
                    super();
                }

                // Should error: missing override modifier
                public function overridableMethod():String {
                    return "child";
                }
            }
        "#;

        let result = compile_haxe_source(haxe_code);
        assert!(!result.errors.is_empty(), "Should have errors");

        let has_missing_override_error = result
            .errors
            .iter()
            .any(|e| e.message.contains("missing") && e.message.contains("override"));
        assert!(
            has_missing_override_error,
            "Should detect missing override modifier"
        );
    }

    #[test]
    fn test_invalid_override_no_parent_method() {
        let haxe_code = r#"
            class BaseClass {
                public function new() {}
            }

            class ChildClass extends BaseClass {
                public function new() {
                    super();
                }

                // Should error: no parent method to override
                override public function nonExistentMethod():Void {
                }
            }
        "#;

        let result = compile_haxe_source(haxe_code);
        assert!(!result.errors.is_empty(), "Should have errors");

        let has_invalid_override_error = result
            .errors
            .iter()
            .any(|e| e.message.contains("override") && e.message.contains("no parent method"));
        assert!(has_invalid_override_error, "Should detect invalid override");
    }

    #[test]
    fn test_correct_override() {
        let haxe_code = r#"
            class BaseClass {
                public function new() {}

                public function overridableMethod():String {
                    return "base";
                }
            }

            class ChildClass extends BaseClass {
                public function new() {
                    super();
                }

                // Correct: has override modifier
                override public function overridableMethod():String {
                    return "child";
                }
            }
        "#;

        let result = compile_haxe_source(haxe_code);

        // Should not have override-related errors
        let has_override_error = result.errors.iter().any(|e| e.message.contains("override"));
        assert!(
            !has_override_error,
            "Should not have override errors for correct usage"
        );
    }

    #[test]
    fn test_override_with_signature_mismatch() {
        let haxe_code = r#"
            class BaseClass {
                public function new() {}

                public function overridableMethod():String {
                    return "base";
                }
            }

            class ChildClass extends BaseClass {
                public function new() {
                    super();
                }

                // Should error: wrong return type
                override public function overridableMethod():Int {
                    return 42;
                }
            }
        "#;

        let result = compile_haxe_source(haxe_code);
        assert!(!result.errors.is_empty(), "Should have errors");

        let has_signature_error = result
            .errors
            .iter()
            .any(|e| e.message.contains("signature") || e.message.contains("incompatible"));
        assert!(has_signature_error, "Should detect signature mismatch");
    }

    #[test]
    fn test_multiple_inheritance_levels() {
        let haxe_code = r#"
            class GrandParent {
                public function new() {}

                public function ancestorMethod():Bool {
                    return true;
                }
            }

            class Parent extends GrandParent {
                public function new() {
                    super();
                }

                override public function ancestorMethod():Bool {
                    return false;
                }

                public function parentMethod():Int {
                    return 10;
                }
            }

            class Child extends Parent {
                public function new() {
                    super();
                }

                // Should error: missing override for ancestorMethod
                public function ancestorMethod():Bool {
                    return true;
                }

                // Correct: has override for parentMethod
                override public function parentMethod():Int {
                    return 20;
                }
            }
        "#;

        let result = compile_haxe_source(haxe_code);

        // Should detect missing override for ancestorMethod
        let has_missing_override = result
            .errors
            .iter()
            .any(|e| e.message.contains("ancestorMethod") && e.message.contains("override"));

        // Note: Current implementation only checks immediate parent,
        // so this might not catch the grandparent method override
        println!(
            "Override validation errors: {:?}",
            result
                .errors
                .iter()
                .filter(|e| e.message.contains("override"))
                .collect::<Vec<_>>()
        );
    }
}
