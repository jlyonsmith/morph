//! A cross-language schema compiler that generates type definitions and serialization code from a simple, declarative schema language.
//! This crate contains the Abstract Syntaxt Tree (AST), errors and parsing code for the Geno tool.

#![warn(missing_docs)]

/// Namespace containing the AST structures
pub mod ast; // Keep the `ast::` module prefixwhen exporting from this crate
mod error;

pub use error::*;
use pest::{Parser as PestParser, iterators::Pair};
use pest_derive::Parser;
use std::{collections::HashMap, path::PathBuf};

// Put the Pest parser in a private module to suppress doc warnings
// See [Issue #326](https://github.com/pest-parser/pest/issues/326)
mod parser {
    use super::*;

    #[derive(Parser)]
    #[grammar = "geno.pest"]
    pub struct GenoParser;
}

use parser::{GenoParser, Rule};

/// A Geno AST builder
pub struct GenoAstBuilder {
    file_path: PathBuf,
}

impl GenoAstBuilder {
    /// Create a new Geno AST builder from a file path.  A file path is required
    /// in order to give meaningful error messages.
    pub fn new(file_path: PathBuf) -> Self {
        GenoAstBuilder { file_path }
    }

    /// Build and validate the AST
    pub fn build(&self) -> Result<ast::Schema, GenoError> {
        let input = std::fs::read_to_string(&self.file_path)?;
        let mut schema_pairs = match GenoParser::parse(Rule::_schema, &input) {
            Ok(pairs) => pairs,
            Err(err) => {
                return Err(GenoError::Parse {
                    content: err.line().to_string(),
                    file: self.file_path.to_string_lossy().into_owned(),
                    location: Location::from(err.line_col),
                });
            }
        };
        let metadata = self.build_meta_decl(schema_pairs.next().unwrap())?;
        let mut declarations = Vec::new();

        while let Some(pair) = schema_pairs.next() {
            if pair.as_rule() == Rule::EOI {
                break;
            }

            let rule = pair.as_rule();
            let declaration = match rule {
                Rule::enum_decl => self.build_enum_decl(pair),
                Rule::struct_decl => self.build_struct_decl(pair),
                _ => {
                    return Err(GenoError::new_parse_error(&pair, &self.file_path));
                }
            }?;

            declarations.push(declaration);
        }

        let schema = ast::Schema {
            metadata,
            declarations,
        };

        schema.validate()?;

        Ok(schema)
    }

    fn build_meta_decl(
        &self,
        pair: Pair<'_, Rule>,
    ) -> Result<HashMap<String, ast::MetadataValue>, GenoError> {
        let mut inner_pairs = pair.into_inner();
        let inner_pair = inner_pairs.next().unwrap();
        let mut metadata = HashMap::new();

        // Parse 'meta_data_entry' pairs
        for entry_pair in inner_pair.into_inner() {
            let mut inner_pairs = entry_pair.into_inner();
            let ident = inner_pairs.next().unwrap().as_str().to_string();
            let value_pair = inner_pairs.next().unwrap();
            let value = match value_pair.as_rule() {
                Rule::string_literal => ast::MetadataValue::String(value_pair.as_str().to_string()),
                Rule::integer_literal => {
                    ast::MetadataValue::Integer(self.build_integer_literal(value_pair)?)
                }
                _ => {
                    return Err(GenoError::new_parse_error(&value_pair, &self.file_path));
                }
            };

            metadata.insert(ident, value);
        }

        Ok(metadata)
    }

    fn build_integer_type(&self, pair: Pair<'_, Rule>) -> Result<ast::IntegerType, GenoError> {
        let s = pair.as_str();

        match s {
            "i8" => Ok(ast::IntegerType::I8),
            "u8" => Ok(ast::IntegerType::U8),
            "i16" => Ok(ast::IntegerType::I16),
            "u16" => Ok(ast::IntegerType::U16),
            "i32" => Ok(ast::IntegerType::I32),
            "u32" => Ok(ast::IntegerType::U32),
            "i64" => Ok(ast::IntegerType::I64),
            "u64" => Ok(ast::IntegerType::U64),
            _ => Err(GenoError::new_parse_error(&pair, &self.file_path)),
        }
    }

    fn build_integer_literal(&self, pair: Pair<'_, Rule>) -> Result<ast::IntegerValue, GenoError> {
        let s = pair.as_str();

        if s.starts_with("0x") {
            // Hexadecimal
            let value = u64::from_str_radix(&s[2..], 16)
                .map_err(|_| GenoError::new_number_format_error(&pair, &self.file_path))?;
            Ok(ast::IntegerValue::U64(value))
        } else if s.starts_with("0b") {
            // Binary
            let value = u64::from_str_radix(&s[2..], 2)
                .map_err(|_| GenoError::new_number_format_error(&pair, &self.file_path))?;
            Ok(ast::IntegerValue::U64(value))
        } else {
            // Decimal
            let value = s
                .parse::<i64>()
                .map_err(|_| GenoError::new_number_format_error(&pair, &self.file_path))?;
            Ok(ast::IntegerValue::I64(value))
        }
    }

    fn build_enum_decl<'a>(
        &self,
        enum_decl_pair: Pair<'a, Rule>,
    ) -> Result<ast::Declaration, GenoError> {
        let mut inner_pairs = enum_decl_pair.into_inner();

        let ident = inner_pairs.next().unwrap().as_str().to_string();
        let mut next_pair = inner_pairs.next().unwrap();
        let base_type;

        if next_pair.as_rule() == Rule::integer_type {
            base_type = self.build_integer_type(next_pair)?;
            next_pair = inner_pairs.next().unwrap();
        } else {
            // No base type specified, default to i32
            base_type = ast::IntegerType::I32
        };

        // next_pair is now an 'enum_variant_list'
        let mut variants: Vec<(String, Option<ast::IntegerValue>)> = Vec::new();

        for enum_variant_pair in next_pair.into_inner() {
            let mut variant_inner = enum_variant_pair.into_inner();
            let variant_ident = variant_inner.next().unwrap().as_str().to_string();
            let variant_value = if let Some(value_pair) = variant_inner.next() {
                Some(self.build_integer_literal(value_pair)?)
            } else {
                None
            };
            variants.push((variant_ident, variant_value));
        }

        Ok(ast::Declaration::Enum {
            ident,
            base_type,
            variants,
        })
    }

    fn build_struct_decl<'a>(
        &self,
        struct_decl_pair: Pair<'a, Rule>,
    ) -> Result<ast::Declaration, GenoError> {
        let mut inner_pairs = struct_decl_pair.into_inner();

        let ident = inner_pairs.next().unwrap().as_str().to_string();
        let next_pair = inner_pairs.next().unwrap();

        // next_pair is now a 'struct_field_list'
        let mut fields: Vec<(String, ast::FieldType)> = Vec::new();

        for struct_field_pair in next_pair.into_inner() {
            let mut struct_field_inner = struct_field_pair.into_inner();
            let field_ident = struct_field_inner.next().unwrap().as_str().to_string();

            fields.push((
                field_ident,
                self.build_field_type(struct_field_inner.next().unwrap())?,
            ));
        }

        // Parse struct declaration
        Ok(ast::Declaration::Struct { ident, fields })
    }

    fn build_field_type<'a>(&self, pair: Pair<'a, Rule>) -> Result<ast::FieldType, GenoError> {
        let mut inner_pairs = pair.into_inner();
        let inner_pair = inner_pairs.next().unwrap();

        let nullable = if let Some(nullable_pair) = inner_pairs.peek() {
            if nullable_pair.as_rule() == Rule::nullable {
                true
            } else {
                false
            }
        } else {
            false
        };

        match inner_pair.as_rule() {
            Rule::array_type => {
                let mut inner_pairs = inner_pair.into_inner();
                let element_type_pair = inner_pairs.next().unwrap();
                let length = if let Some(length_pair) = inner_pairs.next() {
                    Some(length_pair.as_str().parse::<usize>().map_err(|_| {
                        GenoError::new_number_range_error(&length_pair, &self.file_path)
                    })?)
                } else {
                    None
                };
                Ok(ast::FieldType::Array(
                    Box::new(self.build_field_type(element_type_pair)?),
                    length,
                    nullable,
                ))
            }
            Rule::map_type => {
                let mut inner_pairs = inner_pair.into_inner();
                let key_type_pair = inner_pairs.next().unwrap();
                let value_type_pair = inner_pairs.next().unwrap();

                Ok(ast::FieldType::Map(
                    self.build_builtin_type(key_type_pair)?,
                    Box::new(self.build_field_type(value_type_pair)?),
                    nullable,
                ))
            }
            Rule::builtin_type => Ok(ast::FieldType::Builtin(
                self.build_builtin_type(inner_pair)?,
                nullable,
            )),
            Rule::identifier => Ok(ast::FieldType::UserDefined(
                inner_pair.as_str().to_string(),
                nullable,
            )),
            _ => Err(GenoError::new_parse_error(&inner_pair, &self.file_path)),
        }
    }

    fn build_builtin_type(&self, pair: Pair<'_, Rule>) -> Result<ast::BuiltinType, GenoError> {
        let mut inner_pairs = pair.into_inner();
        let inner_pair = inner_pairs.next().unwrap();

        match inner_pair.as_rule() {
            Rule::integer_type => self
                .build_integer_type(inner_pair)
                .map(ast::BuiltinType::Integer),
            Rule::float_type => {
                let s = inner_pair.as_str();
                match s {
                    "f32" => Ok(ast::BuiltinType::Float(ast::FloatType::F32)),
                    "f64" => Ok(ast::BuiltinType::Float(ast::FloatType::F64)),
                    _ => Err(GenoError::new_parse_error(&inner_pair, &self.file_path)),
                }
            }
            Rule::string_type => Ok(ast::BuiltinType::String),
            Rule::bool_type => Ok(ast::BuiltinType::Bool),
            _ => Err(GenoError::new_parse_error(&inner_pair, &self.file_path)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::NamedTempFile;

    #[test]
    fn happy_path() {
        let input = r#"
meta { version = 1 }
enum enum1: i16 {
    default = -1,
    apple = 1,
    orange = 2,
    kiwiFruit = 3,
    pear, // = 3 is inferred
}
// Another comment
struct type1 {
    alpha: i8,
    alpha_beta: u8,
    alphaBeta: i16,
    a4: u16,
    a5: i32,
    a6: u32,
    a7: i64,
    a8: u64,
    a9: f32,
    a10: f64,
    n1: i8?,
    n2: u8?,
    n3: i16?,
    n4: u16?,
    n5: i16?,
    n6: u16?,
    n7: i32?,
    n8: u32?,
    n9: i64?,
    n10: u64?,
    s1: string,
    s2: string?,
    b1: bool,
    b2: bool?,
    e1: enum1,
    e2: enum1?,
    r1: [ string ],
    r2: [ string ]?,
    r3: [ string; 10],
    m1: { string : f64 },
    m2: { string : string },
    m3: { string : bool },
    t1: type1,
};"#;
        let file = NamedTempFile::new().unwrap();
        let path = file.path().to_path_buf();
        fs::write(&path, input).unwrap();
        let ast = GenoAstBuilder::new(path).build().unwrap();

        println!("{:?}", ast);
    }
}
