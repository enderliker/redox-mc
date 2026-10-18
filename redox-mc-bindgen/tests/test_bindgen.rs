#![allow(clippy::all)]

#[path = "generated/block_pos.rs"]
#[allow(clippy::all)]
mod block_pos;
#[path = "generated/direction.rs"]
#[allow(clippy::all)]
mod direction;
#[path = "generated/identifier.rs"]
#[allow(clippy::all)]
mod identifier;
#[path = "generated/resource_key.rs"]
#[allow(clippy::all)]
mod resource_key;

pub use block_pos::BlockPos;
pub use direction::Direction;
pub use identifier::Identifier;
pub use resource_key::ResourceKey;

#[test]
fn test_generated_identifier_struct_properties() {
    assert_eq!(Identifier::CLASS_NAME, "net/minecraft/resources/Identifier");
    let handle = Identifier::from_raw(std::ptr::null_mut());
    assert!(handle.as_raw().is_null());
}

#[test]
fn test_generated_resource_key_struct_properties() {
    assert_eq!(
        ResourceKey::CLASS_NAME,
        "net/minecraft/resources/ResourceKey"
    );
    let handle = ResourceKey::from_raw(std::ptr::null_mut());
    assert!(handle.as_raw().is_null());
}

#[test]
fn test_generated_block_pos_struct_properties() {
    assert_eq!(BlockPos::CLASS_NAME, "net/minecraft/core/BlockPos");
    let handle = BlockPos::from_raw(std::ptr::null_mut());
    assert!(handle.as_raw().is_null());
}

#[test]
fn test_generated_direction_struct_properties() {
    assert_eq!(Direction::CLASS_NAME, "net/minecraft/core/Direction");
    let handle = Direction::from_raw(std::ptr::null_mut());
    assert!(handle.as_raw().is_null());
}

#[test]
fn test_programmatic_generation_and_caching_code() {
    let candidates = [
        std::path::Path::new("versions/26.3/server-26.3.jar"),
        std::path::Path::new("/home/ender/server-inner/server-26.3.jar"),
        std::path::Path::new("../versions/26.3/server-26.3.jar"),
    ];

    let jar_path = candidates.iter().find(|p| p.exists());
    if jar_path.is_none() {
        println!("SKIPPED: server-26.3.jar not found");
        return;
    }

    let model = redox_mc_bindgen::read_class_from_jar(
        jar_path.unwrap(),
        "net/minecraft/resources/Identifier",
    )
    .expect("Read class");
    let code = redox_mc_bindgen::emit_class(&model);

    // Verify acceptance criteria in generated code:
    // 1. Rust struct wrapping JNI jobject
    assert!(code.contains("pub struct Identifier"));
    assert!(code.contains("pub raw: jni::jobject"));
    // 2. Class GlobalRef caching
    assert!(code.contains("CLASS_REF: AtomicPtr<std::ffi::c_void>"));
    assert!(code.contains("NewGlobalRef"));
    // 3. MethodID caching
    assert!(code.contains("METHOD_ID: AtomicPtr<std::ffi::c_void>"));
    assert!(code.contains("GetMethodID"));
    assert!(code.contains("GetStaticMethodID"));
    // 4. FieldID caching
    assert!(code.contains("FIELD_ID: AtomicPtr<std::ffi::c_void>"));
    assert!(code.contains("GetStaticFieldID"));
    // 5. Encapsulated JNI method calls
    assert!(code.contains("CallStaticObjectMethodA"));
    assert!(code.contains("CallObjectMethodA"));
    assert!(code.contains("get_path"));
    assert!(code.contains("from_namespace_and_path"));
    assert!(code.contains("try_parse"));
}
