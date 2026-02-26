use crate::error::*;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Enum representing integer types
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum IntegerType {
    /// Signed 8-bit integer
    I8,
    /// Signed 16-bit integer
    I16,
    /// Signed 32-bit integer
    I32,
    /// Signed 64-bit integer
    I64,
    /// Unsigned 8-bit integer
    U8,
    /// Unsigned 16-bit integer
    U16,
    /// Unsigned 32-bit integer
    U32,
    /// Unsigned 64-bit integer
    U64,
}

/// Enum representing integer values
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum IntegerValue {
    /// Signed 8-bit integer value
    I8(i8),
    /// Signed 16-bit integer value
    I16(i16),
    /// Signed 32-bit integer value
    I32(i32),
    /// Signed 64-bit integer value
    I64(i64),
    /// Unsigned 8-bit integer value
    U8(u8),
    /// Unsigned 16-bit integer value
    U16(u16),
    /// Unsigned 32-bit integer value
    U32(u32),
    /// Unsigned 64-bit integer value
    U64(u64),
}

/// Enum representing float values
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FloatType {
    /// 32-bit floating-point value
    F32,
    /// 64-bit floating-point value
    F64,
}

/// Enum representing all built-in types
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum BuiltinType {
    /// Integer types
    Integer(IntegerType),
    /// Float types
    Float(FloatType),
    /// String type
    String,
    /// Bool type
    Bool,
}

/// Enum representing all field types
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FieldType {
    /// Array type
    Array(Box<FieldType>, Option<usize>, bool),
    /// Map type
    Map(BuiltinType, Box<FieldType>, bool),
    /// Builtin type
    Builtin(BuiltinType, bool),
    /// User-defined type
    UserDefined(String, bool),
}

/// Enum representing metadata values
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum MetadataValue {
    /// String value
    String(String),
    /// Integer value
    Integer(IntegerValue),
}

/// Enum representing declarations
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Declaration {
    /// Enum declaration
    Enum {
        /// Enum identifier
        ident: String,
        /// Enum base integer type
        base_type: IntegerType,
        /// Enum variants
        variants: Vec<(String, Option<IntegerValue>)>,
    },
    /// Struct declaration
    Struct {
        /// Struct identifier
        ident: String,
        /// Struct fields
        fields: Vec<(String, FieldType)>,
    },
}

/// Schema declaration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Schema {
    /// Schema metadata
    pub metadata: HashMap<String, MetadataValue>,
    /// Schema declarations
    pub declarations: Vec<Declaration>,
}

impl Schema {
    /// Validate the schema, checking for duplicate type definitions and duplicate fields/variants within each declaration
    pub fn validate(&self) -> Result<(), GenoError> {
        let mut type_names = HashSet::new();

        // Check for duplicate type definitions and duplicate fields/variants within each declaration
        for decl in &self.declarations {
            match decl {
                Declaration::Enum {
                    ident, variants, ..
                } => {
                    if !type_names.insert(ident.as_str()) {
                        return Err(GenoError::DuplicateType(ident.clone()));
                    }
                    let mut variant_names = HashSet::new();

                    for (variant_name, _) in variants {
                        if !variant_names.insert(variant_name.as_str()) {
                            return Err(GenoError::DuplicateVariant(
                                ident.clone(),
                                variant_name.clone(),
                            ));
                        }
                    }
                }
                Declaration::Struct { ident, fields } => {
                    if !type_names.insert(ident.as_str()) {
                        return Err(GenoError::DuplicateType(ident.clone()));
                    }
                    let mut field_names = HashSet::new();

                    for (field_name, _) in fields {
                        if !field_names.insert(field_name.as_str()) {
                            return Err(GenoError::DuplicateField(
                                ident.clone(),
                                field_name.clone(),
                            ));
                        }
                    }
                }
            }
        }

        // Check for undefined user-defined types
        for decl in &self.declarations {
            if let Declaration::Struct { fields, .. } = decl {
                for (_, field_type) in fields {
                    self.check_undefined_types(field_type, &type_names)?;
                }
            }
        }

        Ok(())
    }

    fn check_undefined_types(
        &self,
        field_type: &FieldType,
        type_names: &HashSet<&str>,
    ) -> Result<(), GenoError> {
        match field_type {
            FieldType::UserDefined(name, _) => {
                if !type_names.contains(name.as_str()) {
                    return Err(GenoError::UndefinedType(name.clone()));
                }
            }
            FieldType::Array(inner, _, _) => {
                self.check_undefined_types(inner, type_names)?;
            }
            FieldType::Map(_, value_type, _) => {
                self.check_undefined_types(value_type, type_names)?;
            }
            FieldType::Builtin(_, _) => {}
        }
        Ok(())
    }
}
