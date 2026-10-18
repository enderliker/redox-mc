pub mod registry_hook;
pub mod wrapper;

use redox_mc_adapter_api::HookTarget;
use ristretto_classfile::ClassFile;
pub use wrapper::{apply_wrapper_hooks, TransformerError};

/// Transforms a Java classfile if it matches any hook in the active hook table.
///
/// Returns `Ok(Some(transformed_bytes))` if transformations were successfully applied,
/// or `Ok(None)` if no hook matched or no modification was required.
pub fn transform_class(
    class_bytes: &[u8],
    hooks: &[HookTarget],
) -> Result<Option<Vec<u8>>, TransformerError> {
    let mut class_file = ClassFile::from_bytes(class_bytes)?;
    let Ok(class_name) = class_file.class_name() else {
        return Ok(None);
    };

    // Filter hooks matching this class name
    let matching_hooks: Vec<HookTarget> = hooks
        .iter()
        .filter(|h| class_name == h.class_name)
        .cloned()
        .collect();

    if matching_hooks.is_empty() {
        return Ok(None);
    }

    let modified = apply_wrapper_hooks(&mut class_file, &matching_hooks)?;
    if !modified {
        return Ok(None);
    }

    let mut output = Vec::new();
    class_file.to_bytes(&mut output)?;
    Ok(Some(output))
}
