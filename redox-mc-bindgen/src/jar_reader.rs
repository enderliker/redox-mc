use crate::model::{ClassModel, FieldModel, JvmType, MethodModel, MethodSignature};
use ristretto_classfile::{ClassAccessFlags, ClassFile, FieldAccessFlags, MethodAccessFlags};
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;
use thiserror::Error;
use zip::ZipArchive;

#[derive(Debug, Error)]
pub enum JarReaderError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("ZIP archive error: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("Classfile parsing error: {0}")]
    ClassFile(#[from] ristretto_classfile::Error),
    #[error("Class '{0}' not found in archive '{1}'")]
    ClassNotFound(String, String),
    #[error("Invalid descriptor in class '{0}': {1}")]
    Descriptor(String, String),
}

pub fn read_class_from_jar(
    jar_path: &Path,
    class_query: &str,
) -> Result<ClassModel, JarReaderError> {
    let trimmed = class_query.trim();
    let stripped = trimmed.strip_suffix(".class").unwrap_or(trimmed);
    let normalized_class = stripped.replace('.', "/");

    let entry_name = format!("{}.class", normalized_class);

    let file = File::open(jar_path)?;
    let reader = BufReader::new(file);
    let mut archive = ZipArchive::new(reader)?;

    let mut class_bytes = Vec::new();
    {
        let mut zip_entry = archive.by_name(&entry_name).map_err(|_| {
            JarReaderError::ClassNotFound(class_query.to_string(), jar_path.display().to_string())
        })?;
        zip_entry
            .by_ref()
            .take(16 * 1024 * 1024)
            .read_to_end(&mut class_bytes)?;
    }

    parse_class_bytes(&class_bytes)
}

pub fn parse_class_bytes(bytes: &[u8]) -> Result<ClassModel, JarReaderError> {
    let class_file = ClassFile::from_bytes(bytes)?;

    let class_name = class_file
        .class_name()
        .map_err(JarReaderError::ClassFile)?
        .to_string();

    let simple_name = class_name
        .rsplit('/')
        .next()
        .unwrap_or(&class_name)
        .to_string();

    let super_class = if class_file.super_class != 0 {
        class_file
            .constant_pool
            .try_get_class(class_file.super_class)
            .ok()
            .and_then(|s| s.as_str().map(|v| v.to_string()))
    } else {
        None
    };

    let mut interfaces = Vec::new();
    for interface_idx in &class_file.interfaces {
        if let Ok(name) = class_file.constant_pool.try_get_class(*interface_idx) {
            if let Some(s) = name.as_str() {
                interfaces.push(s.to_string());
            }
        }
    }

    let is_public = class_file.access_flags.contains(ClassAccessFlags::PUBLIC);
    let is_final = class_file.access_flags.contains(ClassAccessFlags::FINAL);
    let is_interface = class_file
        .access_flags
        .contains(ClassAccessFlags::INTERFACE);

    let mut fields = Vec::new();
    for field in &class_file.fields {
        let Ok(name) = class_file.constant_pool.try_get_utf8(field.name_index) else {
            continue;
        };
        let Ok(desc) = class_file
            .constant_pool
            .try_get_utf8(field.descriptor_index)
        else {
            continue;
        };

        let Some(name_str) = name.as_str() else {
            continue;
        };
        let Some(desc_str) = desc.as_str() else {
            continue;
        };

        // Skip synthetic fields and internal compiler artifacts (e.g. $assertionsDisabled)
        if field.access_flags.contains(FieldAccessFlags::SYNTHETIC) || name_str.starts_with('$') {
            continue;
        }

        let (jvm_type, _) = JvmType::parse(desc_str).map_err(|e| {
            JarReaderError::Descriptor(class_name.clone(), format!("field {}: {}", name_str, e))
        })?;

        let is_static = field.access_flags.contains(FieldAccessFlags::STATIC);
        let is_final = field.access_flags.contains(FieldAccessFlags::FINAL);
        let is_pub = field.access_flags.contains(FieldAccessFlags::PUBLIC);

        fields.push(FieldModel {
            name: name_str.to_string(),
            descriptor: desc_str.to_string(),
            jvm_type,
            access_flags: field.access_flags,
            is_static,
            is_final,
            is_public: is_pub,
        });
    }

    let mut methods = Vec::new();
    for method in &class_file.methods {
        let Ok(name) = class_file.constant_pool.try_get_utf8(method.name_index) else {
            continue;
        };
        let Ok(desc) = class_file
            .constant_pool
            .try_get_utf8(method.descriptor_index)
        else {
            continue;
        };

        let Some(name_str) = name.as_str() else {
            continue;
        };
        let Some(desc_str) = desc.as_str() else {
            continue;
        };

        // Skip synthetic / compiler-bridge methods and class initializers
        if method.access_flags.contains(MethodAccessFlags::SYNTHETIC)
            || method.access_flags.contains(MethodAccessFlags::BRIDGE)
            || name_str == "<clinit>"
        {
            continue;
        }

        let signature = MethodSignature::parse(desc_str).map_err(|e| {
            JarReaderError::Descriptor(class_name.clone(), format!("method {}: {}", name_str, e))
        })?;

        let is_static = method.access_flags.contains(MethodAccessFlags::STATIC);
        let is_constructor = name_str == "<init>";
        let is_pub = method.access_flags.contains(MethodAccessFlags::PUBLIC);

        methods.push(MethodModel {
            name: name_str.to_string(),
            descriptor: desc_str.to_string(),
            signature,
            access_flags: method.access_flags,
            is_static,
            is_constructor,
            is_public: is_pub,
        });
    }

    Ok(ClassModel {
        name: class_name,
        simple_name,
        super_class,
        interfaces,
        access_flags: class_file.access_flags,
        is_public,
        is_final,
        is_interface,
        fields,
        methods,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_identifier_from_server_jar() {
        let candidates = [
            Path::new("versions/26.3/server-26.3.jar"),
            Path::new("/home/ender/server-inner/server-26.3.jar"),
            Path::new("../versions/26.3/server-26.3.jar"),
        ];

        let jar_path = candidates.iter().find(|p| p.exists());
        if jar_path.is_none() {
            println!("SKIPPED: server-26.3.jar not found");
            return;
        }

        let model = read_class_from_jar(jar_path.unwrap(), "net/minecraft/resources/Identifier")
            .expect("Must parse Identifier from jar");

        assert_eq!(model.name, "net/minecraft/resources/Identifier");
        assert_eq!(model.simple_name, "Identifier");
        assert_eq!(model.super_class.as_deref(), Some("java/lang/Object"));
        assert!(model.is_public);
        assert!(model.is_final);

        // Check fields
        assert!(model.fields.iter().any(|f| f.name == "DEFAULT_NAMESPACE"));
        assert!(model.fields.iter().any(|f| f.name == "NAMESPACE_SEPARATOR"));

        // Check methods
        assert!(model
            .methods
            .iter()
            .any(|m| m.name == "getPath" && !m.is_static));
        assert!(model
            .methods
            .iter()
            .any(|m| m.name == "getNamespace" && !m.is_static));
        assert!(model
            .methods
            .iter()
            .any(|m| m.name == "fromNamespaceAndPath" && m.is_static));
        assert!(model
            .methods
            .iter()
            .any(|m| m.name == "tryParse" && m.is_static));
    }
}
