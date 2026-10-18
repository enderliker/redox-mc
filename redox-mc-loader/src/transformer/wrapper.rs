use redox_mc_adapter_api::{HookTarget, HookTechnique};
use ristretto_classfile::attributes::{Attribute, Instruction, MaxStack, StackFrame};
use ristretto_classfile::{ClassFile, Method, MethodAccessFlags};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum TransformerError {
    #[error("Classfile error: {0}")]
    ClassFile(#[from] ristretto_classfile::Error),
    #[error("Invalid descriptor: {0}")]
    InvalidDescriptor(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JvmType {
    Int,    // Z, B, C, S, I
    Long,   // J (2 slots)
    Float,  // F
    Double, // D (2 slots)
    Object, // L...; or [...]
}

impl JvmType {
    pub fn slots(&self) -> u8 {
        match self {
            JvmType::Long | JvmType::Double => 2,
            _ => 1,
        }
    }
}

/// Parses parameter types and return type from a JVM method descriptor (JVMS §4.3.3).
pub fn parse_descriptor(desc: &str) -> Result<(Vec<JvmType>, Option<JvmType>), TransformerError> {
    if !desc.starts_with('(') {
        return Err(TransformerError::InvalidDescriptor(desc.to_string()));
    }
    let close_paren = desc
        .find(')')
        .ok_or_else(|| TransformerError::InvalidDescriptor(desc.to_string()))?;
    let params_str = &desc[1..close_paren];
    let return_str = &desc[close_paren + 1..];

    let mut params = Vec::new();
    let chars: Vec<char> = params_str.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        match chars[i] {
            'Z' | 'B' | 'C' | 'S' | 'I' => {
                params.push(JvmType::Int);
                i += 1;
            }
            'J' => {
                params.push(JvmType::Long);
                i += 1;
            }
            'F' => {
                params.push(JvmType::Float);
                i += 1;
            }
            'D' => {
                params.push(JvmType::Double);
                i += 1;
            }
            'L' => {
                params.push(JvmType::Object);
                while i < chars.len() && chars[i] != ';' {
                    i += 1;
                }
                if i < chars.len() {
                    i += 1; // consume ';'
                }
            }
            '[' => {
                params.push(JvmType::Object);
                while i < chars.len() && chars[i] == '[' {
                    i += 1;
                }
                if i < chars.len() {
                    if chars[i] == 'L' {
                        while i < chars.len() && chars[i] != ';' {
                            i += 1;
                        }
                        if i < chars.len() {
                            i += 1;
                        }
                    } else {
                        i += 1;
                    }
                }
            }
            _ => return Err(TransformerError::InvalidDescriptor(desc.to_string())),
        }
    }

    let ret = match return_str.chars().next() {
        Some('V') => None,
        Some('Z' | 'B' | 'C' | 'S' | 'I') => Some(JvmType::Int),
        Some('J') => Some(JvmType::Long),
        Some('F') => Some(JvmType::Float),
        Some('D') => Some(JvmType::Double),
        Some('L' | '[') => Some(JvmType::Object),
        _ => return Err(TransformerError::InvalidDescriptor(desc.to_string())),
    };

    Ok((params, ret))
}

fn make_load_instruction(jvm_type: JvmType, slot: u8) -> Instruction {
    match jvm_type {
        JvmType::Int => match slot {
            0 => Instruction::Iload_0,
            1 => Instruction::Iload_1,
            2 => Instruction::Iload_2,
            3 => Instruction::Iload_3,
            s => Instruction::Iload(s),
        },
        JvmType::Long => match slot {
            0 => Instruction::Lload_0,
            1 => Instruction::Lload_1,
            2 => Instruction::Lload_2,
            3 => Instruction::Lload_3,
            s => Instruction::Lload(s),
        },
        JvmType::Float => match slot {
            0 => Instruction::Fload_0,
            1 => Instruction::Fload_1,
            2 => Instruction::Fload_2,
            3 => Instruction::Fload_3,
            s => Instruction::Fload(s),
        },
        JvmType::Double => match slot {
            0 => Instruction::Dload_0,
            1 => Instruction::Dload_1,
            2 => Instruction::Dload_2,
            3 => Instruction::Dload_3,
            s => Instruction::Dload(s),
        },
        JvmType::Object => match slot {
            0 => Instruction::Aload_0,
            1 => Instruction::Aload_1,
            2 => Instruction::Aload_2,
            3 => Instruction::Aload_3,
            s => Instruction::Aload(s),
        },
    }
}

fn make_return_instruction(ret_type: Option<JvmType>) -> Instruction {
    match ret_type {
        None => Instruction::Return,
        Some(JvmType::Int) => Instruction::Ireturn,
        Some(JvmType::Long) => Instruction::Lreturn,
        Some(JvmType::Float) => Instruction::Freturn,
        Some(JvmType::Double) => Instruction::Dreturn,
        Some(JvmType::Object) => Instruction::Areturn,
    }
}

/// Applies wrapper method transformations to `class_file` for all matching hooks.
///
/// Returns `Ok(true)` if at least one method was wrapped, `Ok(false)` otherwise.
pub fn apply_wrapper_hooks(
    class_file: &mut ClassFile<'static>,
    hooks: &[HookTarget],
) -> Result<bool, TransformerError> {
    let mut modified = false;

    for hook in hooks {
        if hook.technique != HookTechnique::Wrapper {
            continue;
        }

        let renamed_name = format!("{}$original", hook.method_name);

        // Check if this method was already wrapped
        let mut target_method_idx = None;
        let mut already_wrapped = false;

        for (idx, method) in class_file.methods.iter().enumerate() {
            let Ok(name) = class_file.constant_pool.try_get_utf8(method.name_index) else {
                continue;
            };
            let Ok(desc) = class_file
                .constant_pool
                .try_get_utf8(method.descriptor_index)
            else {
                continue;
            };

            if name == renamed_name.as_str() {
                already_wrapped = true;
                break;
            }

            if name == hook.method_name && desc == hook.method_desc {
                target_method_idx = Some(idx);
            }
        }

        if already_wrapped || target_method_idx.is_none() {
            continue;
        }

        let orig_idx = target_method_idx.unwrap();
        let orig_access_flags = class_file.methods[orig_idx].access_flags;
        let is_static = orig_access_flags.contains(MethodAccessFlags::STATIC);
        let (params, ret_type) = parse_descriptor(hook.method_desc)?;

        // 1. Rename original method to `<name>$original`
        let renamed_name_idx = class_file.constant_pool.add_utf8(&renamed_name)?;
        class_file.methods[orig_idx].name_index = renamed_name_idx;

        // 2. Add constant pool entries for wrapper
        let wrapper_name_idx = class_file.constant_pool.add_utf8(hook.method_name)?;
        let wrapper_desc_idx = class_file.constant_pool.add_utf8(hook.method_desc)?;

        let bridge_class_idx = class_file.constant_pool.add_class(hook.bridge_class)?;
        let bridge_method_ref = class_file.constant_pool.add_method_ref(
            bridge_class_idx,
            hook.bridge_method,
            hook.bridge_desc,
        )?;

        let orig_method_ref = class_file.constant_pool.add_method_ref(
            class_file.this_class,
            renamed_name.as_str(),
            hook.method_desc,
        )?;

        // 3. Generate instructions for wrapper method
        let mut instructions = Vec::new();
        let (bridge_params, _bridge_ret) = parse_descriptor(hook.bridge_desc)?;
        let is_cancellable = hook.bridge_desc.ends_with(")Z") && ret_type.is_none();

        // Push arguments required by the bridge hook method
        if !bridge_params.is_empty() {
            if !is_static {
                instructions.push(Instruction::Aload_0);
            }
            let num_additional = bridge_params
                .len()
                .saturating_sub(if is_static { 0 } else { 1 });
            let mut param_slot = if is_static { 0u8 } else { 1u8 };
            for param in params.iter().take(num_additional) {
                instructions.push(make_load_instruction(*param, param_slot));
                param_slot += param.slots();
            }
        }

        // Call the bridge hook first
        instructions.push(Instruction::Invokestatic(bridge_method_ref));

        let mut code_attributes = Vec::new();

        if is_cancellable {
            // Placeholder for Ifeq; branch target will point to the Return instruction
            let ifeq_idx = instructions.len();
            instructions.push(Instruction::Ifeq(0));

            // Load parameters for the original method call
            let mut current_slot = 0u8;
            if !is_static {
                instructions.push(Instruction::Aload_0);
                current_slot = 1;
            }

            for param in &params {
                instructions.push(make_load_instruction(*param, current_slot));
                current_slot += param.slots();
            }

            // Invoke the original method
            if is_static {
                instructions.push(Instruction::Invokestatic(orig_method_ref));
            } else {
                instructions.push(Instruction::Invokevirtual(orig_method_ref));
            }

            // Return from the wrapper (target of Ifeq jump when cancelled)
            let target_return_idx = instructions.len() as u16;
            instructions.push(make_return_instruction(ret_type));

            // Update Ifeq instruction with the exact target instruction index
            instructions[ifeq_idx] = Instruction::Ifeq(target_return_idx);

            // In ristretto_classfile's in-memory model, StackFrame offsets are logical
            // instruction indices, which are mapped to bytecode offsets during serialization.
            let smt_frame = StackFrame::SameFrame {
                frame_type: target_return_idx as u8,
            };

            let smt_name_idx = class_file.constant_pool.add_utf8("StackMapTable")?;
            code_attributes.push(Attribute::StackMapTable {
                name_index: smt_name_idx,
                frames: vec![smt_frame],
            });
        } else {
            // If the bridge method returned something and caller expects void, pop it
            if hook.bridge_desc.ends_with(")Z") || hook.bridge_desc.ends_with(")I") {
                instructions.push(Instruction::Pop);
            }

            // Load parameters for the original method call
            let mut current_slot = 0u8;
            if !is_static {
                instructions.push(Instruction::Aload_0);
                current_slot = 1;
            }

            for param in &params {
                instructions.push(make_load_instruction(*param, current_slot));
                current_slot += param.slots();
            }

            // Invoke the original method
            if is_static {
                instructions.push(Instruction::Invokestatic(orig_method_ref));
            } else {
                instructions.push(Instruction::Invokevirtual(orig_method_ref));
            }

            // Invoke optional post-bridge callback
            if let (Some(post_method), Some(post_desc)) =
                (hook.post_bridge_method, hook.post_bridge_desc)
            {
                let post_method_ref = class_file.constant_pool.add_method_ref(
                    bridge_class_idx,
                    post_method,
                    post_desc,
                )?;
                instructions.push(Instruction::Invokestatic(post_method_ref));
                if post_desc.ends_with(")Z") || post_desc.ends_with(")I") {
                    instructions.push(Instruction::Pop);
                }
            }

            // Return from the wrapper
            instructions.push(make_return_instruction(ret_type));
        }

        // 4. Calculate max_stack and max_locals
        let computed_stack = instructions
            .max_stack(&class_file.constant_pool)
            .unwrap_or(4);
        let max_stack = computed_stack.max(2);
        let total_param_slots =
            params.iter().map(|p| p.slots()).sum::<u8>() + if is_static { 0 } else { 1 };
        let max_locals = (total_param_slots as u16).max(if is_static { 0 } else { 1 });

        let code_name_idx = class_file.constant_pool.add_utf8("Code")?;
        let code_attr = Attribute::Code {
            name_index: code_name_idx,
            max_stack,
            max_locals,
            code: instructions,
            exception_table: Vec::new(),
            attributes: code_attributes,
        };

        let wrapper_method = Method {
            access_flags: orig_access_flags,
            name_index: wrapper_name_idx,
            descriptor_index: wrapper_desc_idx,
            attributes: vec![code_attr],
        };

        class_file.methods.push(wrapper_method);
        modified = true;
    }

    Ok(modified)
}
