use ristretto_classfile::{ClassAccessFlags, FieldAccessFlags, MethodAccessFlags};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrimitiveType {
    Boolean, // Z
    Byte,    // B
    Char,    // C
    Short,   // S
    Int,     // I
    Long,    // J
    Float,   // F
    Double,  // D
    Void,    // V
}

impl PrimitiveType {
    pub fn jni_type_name(&self) -> &'static str {
        match self {
            PrimitiveType::Boolean => "jni::jboolean",
            PrimitiveType::Byte => "jni::jbyte",
            PrimitiveType::Char => "jni::jchar",
            PrimitiveType::Short => "jni::jshort",
            PrimitiveType::Int => "jni::jint",
            PrimitiveType::Long => "jni::jlong",
            PrimitiveType::Float => "jni::jfloat",
            PrimitiveType::Double => "jni::jdouble",
            PrimitiveType::Void => "()",
        }
    }

    pub fn jni_call_method_suffix(&self) -> &'static str {
        match self {
            PrimitiveType::Boolean => "BooleanMethod",
            PrimitiveType::Byte => "ByteMethod",
            PrimitiveType::Char => "CharMethod",
            PrimitiveType::Short => "ShortMethod",
            PrimitiveType::Int => "IntMethod",
            PrimitiveType::Long => "LongMethod",
            PrimitiveType::Float => "FloatMethod",
            PrimitiveType::Double => "DoubleMethod",
            PrimitiveType::Void => "VoidMethod",
        }
    }

    pub fn jni_field_suffix(&self) -> &'static str {
        match self {
            PrimitiveType::Boolean => "BooleanField",
            PrimitiveType::Byte => "ByteField",
            PrimitiveType::Char => "CharField",
            PrimitiveType::Short => "ShortField",
            PrimitiveType::Int => "IntField",
            PrimitiveType::Long => "LongField",
            PrimitiveType::Float => "FloatField",
            PrimitiveType::Double => "DoubleField",
            PrimitiveType::Void => panic!("Void cannot be a field type"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JvmType {
    Primitive(PrimitiveType),
    Object(String),      // e.g. "java/lang/String"
    Array(Box<JvmType>), // e.g. "[B"
}

impl JvmType {
    pub fn parse(s: &str) -> Result<(Self, &str), String> {
        if s.is_empty() {
            return Err("Empty descriptor".to_string());
        }
        let first = s.chars().next().unwrap();
        match first {
            'Z' => Ok((JvmType::Primitive(PrimitiveType::Boolean), &s[1..])),
            'B' => Ok((JvmType::Primitive(PrimitiveType::Byte), &s[1..])),
            'C' => Ok((JvmType::Primitive(PrimitiveType::Char), &s[1..])),
            'S' => Ok((JvmType::Primitive(PrimitiveType::Short), &s[1..])),
            'I' => Ok((JvmType::Primitive(PrimitiveType::Int), &s[1..])),
            'J' => Ok((JvmType::Primitive(PrimitiveType::Long), &s[1..])),
            'F' => Ok((JvmType::Primitive(PrimitiveType::Float), &s[1..])),
            'D' => Ok((JvmType::Primitive(PrimitiveType::Double), &s[1..])),
            'V' => Ok((JvmType::Primitive(PrimitiveType::Void), &s[1..])),
            '[' => {
                let (elem, rest) = JvmType::parse(&s[1..])?;
                Ok((JvmType::Array(Box::new(elem)), rest))
            }
            'L' => {
                let end = s
                    .find(';')
                    .ok_or_else(|| format!("Unterminated object descriptor in {}", s))?;
                let class_name = &s[1..end];
                Ok((JvmType::Object(class_name.to_string()), &s[end + 1..]))
            }
            other => Err(format!(
                "Unknown descriptor character: '{}' in {}",
                other, s
            )),
        }
    }

    pub fn rust_type_string(&self) -> String {
        match self {
            JvmType::Primitive(p) => p.jni_type_name().to_string(),
            JvmType::Object(cls) => {
                if cls == "java/lang/String" {
                    "jni::jstring".to_string()
                } else {
                    "jni::jobject".to_string()
                }
            }
            JvmType::Array(_) => "jni::jobject".to_string(),
        }
    }

    pub fn jni_call_method_suffix(&self) -> &'static str {
        match self {
            JvmType::Primitive(p) => p.jni_call_method_suffix(),
            JvmType::Object(_) | JvmType::Array(_) => "ObjectMethod",
        }
    }

    pub fn jni_field_suffix(&self) -> &'static str {
        match self {
            JvmType::Primitive(p) => p.jni_field_suffix(),
            JvmType::Object(_) | JvmType::Array(_) => "ObjectField",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MethodSignature {
    pub raw_descriptor: String,
    pub parameters: Vec<JvmType>,
    pub return_type: JvmType,
}

impl MethodSignature {
    pub fn parse(desc: &str) -> Result<Self, String> {
        if !desc.starts_with('(') {
            return Err(format!(
                "Invalid method descriptor, must start with '(': {}",
                desc
            ));
        }
        let close_paren = desc
            .find(')')
            .ok_or_else(|| format!("Missing ')' in method descriptor: {}", desc))?;
        let mut params_str = &desc[1..close_paren];
        let mut parameters = Vec::new();
        while !params_str.is_empty() {
            let (param, rest) = JvmType::parse(params_str)?;
            parameters.push(param);
            params_str = rest;
        }
        let return_str = &desc[close_paren + 1..];
        let (return_type, rest) = JvmType::parse(return_str)?;
        if !rest.is_empty() {
            return Err(format!(
                "Trailing characters after return type in {}: {}",
                desc, rest
            ));
        }
        Ok(MethodSignature {
            raw_descriptor: desc.to_string(),
            parameters,
            return_type,
        })
    }
}

#[derive(Debug, Clone)]
pub struct FieldModel {
    pub name: String,
    pub descriptor: String,
    pub jvm_type: JvmType,
    pub access_flags: FieldAccessFlags,
    pub is_static: bool,
    pub is_final: bool,
    pub is_public: bool,
}

#[derive(Debug, Clone)]
pub struct MethodModel {
    pub name: String,
    pub descriptor: String,
    pub signature: MethodSignature,
    pub access_flags: MethodAccessFlags,
    pub is_static: bool,
    pub is_constructor: bool,
    pub is_public: bool,
}

#[derive(Debug, Clone)]
pub struct ClassModel {
    pub name: String,        // e.g. "net/minecraft/resources/Identifier"
    pub simple_name: String, // e.g. "Identifier"
    pub super_class: Option<String>,
    pub interfaces: Vec<String>,
    pub access_flags: ClassAccessFlags,
    pub is_public: bool,
    pub is_final: bool,
    pub is_interface: bool,
    pub fields: Vec<FieldModel>,
    pub methods: Vec<MethodModel>,
}
