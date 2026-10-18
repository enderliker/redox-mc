use redox_mc_adapter_26_3::HOOK_TARGETS;
use redox_mc_adapter_api::{HookTarget, HookTechnique};
use redox_mc_loader::transformer::transform_class;
use redox_mc_loader::transformer::wrapper::{parse_descriptor, JvmType};
use ristretto_classfile::ClassFile;
use std::fs;
use std::process::Command;

#[test]
fn test_descriptor_parser() {
    let (params, ret) = parse_descriptor("()V").expect("parse ()V");
    assert!(params.is_empty());
    assert_eq!(ret, None);

    let (params, ret) = parse_descriptor("(IJ)V").expect("parse (IJ)V");
    assert_eq!(params, vec![JvmType::Int, JvmType::Long]);
    assert_eq!(ret, None);

    let (params, ret) =
        parse_descriptor("(Ljava/lang/String;ZD)Ljava/lang/Object;").expect("parse complex");
    assert_eq!(params, vec![JvmType::Object, JvmType::Int, JvmType::Double]);
    assert_eq!(ret, Some(JvmType::Object));

    let (params, ret) = parse_descriptor("([B[[I)Z").expect("parse array");
    assert_eq!(params, vec![JvmType::Object, JvmType::Object]);
    assert_eq!(ret, Some(JvmType::Int));
}

#[test]
fn test_transform_real_minecraft_bootstrap_class() {
    // 1. Extract net/minecraft/server/Bootstrap.class from server-26.3.jar
    let jar_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../versions/26.3/server-26.3.jar"
    );
    let output = Command::new("unzip")
        .args(["-p", jar_path, "net/minecraft/server/Bootstrap.class"])
        .output()
        .expect("Failed to run unzip on server-26.3.jar");

    assert!(
        output.status.success(),
        "unzip net/minecraft/server/Bootstrap.class failed"
    );
    let bootstrap_bytes = output.stdout;
    assert!(
        !bootstrap_bytes.is_empty(),
        "Bootstrap.class bytes must not be empty"
    );

    // 2. Transform Bootstrap class using the 26.3 hook table
    let transformed = transform_class(&bootstrap_bytes, HOOK_TARGETS)
        .expect("Transformation must succeed")
        .expect("Bootstrap must match hook and produce transformed bytes");

    assert_ne!(
        bootstrap_bytes, transformed,
        "Transformed bytes must differ from original"
    );

    // 3. Parse transformed classfile with ristretto_classfile to verify internal integrity
    let class_file =
        ClassFile::from_bytes(&transformed).expect("Transformed classfile must be valid");
    assert_eq!(
        class_file.class_name().unwrap(),
        "net/minecraft/server/Bootstrap"
    );

    // Find original renamed method and new wrapper method
    let mut found_original = false;
    let mut found_wrapper = false;

    for method in &class_file.methods {
        let name = class_file
            .constant_pool
            .try_get_utf8(method.name_index)
            .unwrap();
        let desc = class_file
            .constant_pool
            .try_get_utf8(method.descriptor_index)
            .unwrap();

        if name == "bootStrap$original" && desc == "()V" {
            found_original = true;
        }
        if name == "bootStrap" && desc == "()V" {
            found_wrapper = true;
        }
    }

    assert!(
        found_original,
        "Method 'bootStrap$original()V' must exist in transformed class"
    );
    assert!(
        found_wrapper,
        "Wrapper method 'bootStrap()V' must exist in transformed class"
    );

    // 4. Write transformed class to a temp file and validate with javap -v -p
    let temp_dir = std::env::temp_dir().join("redox_test_bootstrap");
    fs::create_dir_all(&temp_dir).unwrap();
    let temp_class = temp_dir.join("Bootstrap.class");
    fs::write(&temp_class, &transformed).unwrap();

    let javap_output = Command::new("javap")
        .args(["-v", "-p", temp_class.to_str().unwrap()])
        .output()
        .expect("Failed to execute javap");

    assert!(
        javap_output.status.success(),
        "javap failed with error: {}",
        String::from_utf8_lossy(&javap_output.stderr)
    );

    let javap_stdout = String::from_utf8_lossy(&javap_output.stdout);
    assert!(
        javap_stdout.contains("bootStrap$original"),
        "javap output must contain bootStrap$original"
    );
    assert!(
        javap_stdout.contains("onBootstrapEntry"),
        "javap output must contain RedoxBridge.onBootstrapEntry"
    );
    println!("[TEST] javap verification on transformed Bootstrap.class passed successfully!");
}

#[test]
fn test_transform_custom_java_class_with_parameters_and_return() {
    let temp_dir = std::env::temp_dir().join("redox_test_custom");
    fs::create_dir_all(&temp_dir).unwrap();

    // 1. Create a custom Java 25 source file
    let java_src = r#"
package test.sample;

public class CustomTarget {
    public static int compute(int a, long b) {
        return (int) (a + b);
    }
}
"#;
    let java_file = temp_dir.join("CustomTarget.java");
    fs::write(&java_file, java_src).unwrap();

    // Compile with javac
    let javac_status = Command::new("javac")
        .args([
            "-d",
            temp_dir.to_str().unwrap(),
            java_file.to_str().unwrap(),
        ])
        .status()
        .expect("Failed to run javac");
    assert!(javac_status.success(), "javac compilation failed");

    let class_file_path = temp_dir.join("test/sample/CustomTarget.class");
    let original_bytes = fs::read(&class_file_path).expect("Read compiled class");

    // 2. Define custom hook
    let custom_hooks = [HookTarget {
        class_name: "test/sample/CustomTarget",
        method_name: "compute",
        method_desc: "(IJ)I",
        technique: HookTechnique::Wrapper,
        bridge_class: "net/redoxmc/bridge/RedoxBridge",
        bridge_method: "onBootstrapEntry",
        bridge_desc: "()V",
    }];

    // 3. Transform
    let transformed = transform_class(&original_bytes, &custom_hooks)
        .expect("Transform must succeed")
        .expect("Must produce transformed class");

    let transformed_file = temp_dir.join("test/sample/CustomTargetTransformed.class");
    fs::write(&transformed_file, &transformed).unwrap();

    // 4. Validate with javap
    let javap_res = Command::new("javap")
        .args(["-v", "-p", transformed_file.to_str().unwrap()])
        .output()
        .expect("javap command failed");

    assert!(
        javap_res.status.success(),
        "javap verification failed: {}",
        String::from_utf8_lossy(&javap_res.stderr)
    );

    let javap_out = String::from_utf8_lossy(&javap_res.stdout);
    assert!(
        javap_out.contains("compute$original"),
        "javap output must contain compute$original"
    );
    assert!(
        javap_out.contains("ireturn"),
        "javap output must contain ireturn for int return type"
    );
    assert!(
        javap_out.contains("iload_0"),
        "javap output must contain iload_0"
    );
    assert!(
        javap_out.contains("lload_1"),
        "javap output must contain lload_1"
    );
    println!("[TEST] javap verification on custom class passed successfully!");
}
