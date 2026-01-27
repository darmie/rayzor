use parser::parse_haxe_file;

#[test]
fn test_simple_recursive_constraint() {
    let source = r#"
class Comparable<T:Comparable<T>> {
    function compareTo(other:T):Int;
}
"#;

    let result = parse_haxe_file("test.hx", source, false);
    assert!(result.is_ok(), "Parse failed: {:?}", result);

    let file = result.unwrap();
    assert_eq!(file.declarations.len(), 1);

    match &file.declarations[0] {
        parser::haxe_ast::TypeDeclaration::Class(class) => {
            assert_eq!(class.name, "Comparable");
            assert_eq!(class.type_params.len(), 1);
            let type_param = &class.type_params[0];
            assert_eq!(type_param.name, "T");
            assert_eq!(type_param.constraints.len(), 1);

            // Check if constraint is Comparable<T>
            match &type_param.constraints[0] {
                parser::haxe_ast::Type::Path { path, params, .. } => {
                    assert_eq!(path.name, "Comparable");
                    assert_eq!(params.len(), 1);
                    match &params[0] {
                        parser::haxe_ast::Type::Path {
                            path: inner_path,
                            params: inner_params,
                            ..
                        } => {
                            assert_eq!(inner_path.name, "T");
                            assert_eq!(inner_params.len(), 0);
                        }
                        _ => panic!("Expected type parameter T in constraint"),
                    }
                }
                _ => panic!("Expected path type for constraint"),
            }
        }
        _ => panic!("Expected class declaration"),
    }
}

#[test]
fn test_mutual_recursive_constraints() {
    let source = r#"
interface Container<T:Element<U>, U:Container<T, U>> {
    function getElement():T;
}

interface Element<C:Container<?, C>> {
    function getContainer():C;
}
"#;

    let result = parse_haxe_file("test.hx", source, false);
    assert!(result.is_ok(), "Parse failed: {:?}", result);

    let file = result.unwrap();
    assert_eq!(file.declarations.len(), 2);

    // Check Container interface
    match &file.declarations[0] {
        parser::haxe_ast::TypeDeclaration::Interface(interface) => {
            assert_eq!(interface.name, "Container");
            assert_eq!(interface.type_params.len(), 2);

            // Check T constraint (T:Element<U>)
            let t_param = &interface.type_params[0];
            assert_eq!(t_param.name, "T");
            assert_eq!(t_param.constraints.len(), 1);

            // Check U constraint (U:Container<T, U>)
            let u_param = &interface.type_params[1];
            assert_eq!(u_param.name, "U");
            assert_eq!(u_param.constraints.len(), 1);
        }
        _ => panic!("Expected interface declaration"),
    }

    // Check Element interface with wildcard type
    match &file.declarations[1] {
        parser::haxe_ast::TypeDeclaration::Interface(interface) => {
            assert_eq!(interface.name, "Element");
            assert_eq!(interface.type_params.len(), 1);

            // Check C constraint (C:Container<?, C>)
            let c_param = &interface.type_params[0];
            assert_eq!(c_param.name, "C");
            assert_eq!(c_param.constraints.len(), 1);

            match &c_param.constraints[0] {
                parser::haxe_ast::Type::Path { path, params, .. } => {
                    assert_eq!(path.name, "Container");
                    assert_eq!(params.len(), 2);
                    // First param should be wildcard
                    match &params[0] {
                        parser::haxe_ast::Type::Wildcard { .. } => {
                            // Good, it's a wildcard
                        }
                        _ => panic!("Expected wildcard type in Container<?, C>"),
                    }
                }
                _ => panic!("Expected path type for constraint"),
            }
        }
        _ => panic!("Expected interface declaration"),
    }
}

#[test]
fn test_nested_recursive_constraints() {
    let source = r#"
class Node<T:Node<T>> {
    var parent:T;
    var children:Array<T>;
}

class Tree<N:Node<N>> {
    var root:N;
}
"#;

    let result = parse_haxe_file("test.hx", source, false);
    assert!(result.is_ok(), "Parse failed: {:?}", result);
}

#[test]
fn test_complex_recursive_constraint_chain() {
    let source = r#"
interface A<T:B<T, U>, U:C<U>> {}
interface B<T:A<T, U>, U:C<U>> {}
interface C<T:C<T>> {}
"#;

    let result = parse_haxe_file("test.hx", source, false);
    assert!(result.is_ok(), "Parse failed: {:?}", result);
}

#[test]
fn test_recursive_constraint_with_multiple_params() {
    let source = r#"
class Graph<V:Vertex<V, E>, E:Edge<V, E>> {
    var vertices:Array<V>;
    var edges:Array<E>;
}

interface Vertex<V:Vertex<V, E>, E:Edge<V, E>> {
    function getEdges():Array<E>;
}

interface Edge<V:Vertex<V, E>, E:Edge<V, E>> {
    function getStart():V;
    function getEnd():V;
}
"#;

    let result = parse_haxe_file("test.hx", source, false);
    assert!(result.is_ok(), "Parse failed: {:?}", result);
}
