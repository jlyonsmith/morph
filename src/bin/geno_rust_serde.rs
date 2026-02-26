use anyhow::Context;
use geno::ast;
use std::fmt::Write as _;
use std::io::{self, Read};

fn main() {
    if let Err(err) = run() {
        eprintln!("error: {err:#}");
        std::process::exit(1);
    }

    std::process::exit(0);
}

fn run() -> anyhow::Result<()> {
    let stdin = io::stdin();
    let mut handle = stdin.lock();
    let mut buffer = Vec::new();

    // Read all bytes from stdin into the buffer
    handle
        .read_to_end(&mut buffer)
        .context("Unable to read AST from stdin")?;

    let schema: ast::Schema =
        rmp_serde::from_slice(&buffer).context("Unable to deserialize AST from stdin")?;

    let output = generate(&schema);
    print!("{}", output);

    Ok(())
}

fn generate(schema: &ast::Schema) -> String {
    let mut out = String::new();

    writeln!(out, "#![allow(unused_imports)]").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "use serde::{{Deserialize, Serialize}};").unwrap();
    writeln!(out, "use std::collections::HashMap;").unwrap();

    for decl in &schema.declarations {
        writeln!(out).unwrap();
        match decl {
            ast::Declaration::Enum {
                ident,
                base_type,
                variants,
            } => generate_enum(&mut out, ident, base_type, variants),
            ast::Declaration::Struct { ident, fields } => generate_struct(&mut out, ident, fields),
        }
    }

    out
}

fn generate_enum(
    out: &mut String,
    ident: &str,
    base_type: &ast::IntegerType,
    variants: &[(String, Option<ast::IntegerValue>)],
) {
    let rust_name = to_pascal_case(ident);

    writeln!(
        out,
        "#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]"
    )
    .unwrap();
    writeln!(out, "#[repr({})]", integer_type_str(base_type)).unwrap();
    writeln!(out, "pub enum {rust_name} {{").unwrap();

    for (variant_name, value) in variants {
        let rust_variant = to_pascal_case(variant_name);
        if rust_variant != *variant_name {
            writeln!(out, "    #[serde(rename = \"{variant_name}\")]").unwrap();
        }
        match value {
            Some(val) => writeln!(out, "    {rust_variant} = {},", integer_value_str(val)).unwrap(),
            None => writeln!(out, "    {rust_variant},").unwrap(),
        }
    }

    writeln!(out, "}}").unwrap();
}

fn generate_struct(out: &mut String, ident: &str, fields: &[(String, ast::FieldType)]) {
    let rust_name = to_pascal_case(ident);

    writeln!(
        out,
        "#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]"
    )
    .unwrap();
    writeln!(out, "pub struct {rust_name} {{").unwrap();

    for (field_name, field_type) in fields {
        let rust_field = to_snake_case(field_name);
        if rust_field != *field_name {
            writeln!(out, "    #[serde(rename = \"{field_name}\")]").unwrap();
        }
        writeln!(out, "    pub {rust_field}: {},", field_type_str(field_type)).unwrap();
    }

    writeln!(out, "}}").unwrap();
}

fn field_type_str(ft: &ast::FieldType) -> String {
    match ft {
        ast::FieldType::Builtin(bt, nullable) => {
            let base = builtin_type_str(bt);
            if *nullable {
                format!("Option<{base}>")
            } else {
                base
            }
        }
        ast::FieldType::UserDefined(name, nullable) => {
            let rust_name = to_pascal_case(name);
            if *nullable {
                format!("Option<{rust_name}>")
            } else {
                rust_name
            }
        }
        ast::FieldType::Array(inner, length, nullable) => {
            let inner_str = field_type_str(inner);
            let base = match length {
                Some(len) => format!("[{inner_str}; {len}]"),
                None => format!("Vec<{inner_str}>"),
            };
            if *nullable {
                format!("Option<{base}>")
            } else {
                base
            }
        }
        ast::FieldType::Map(key_type, value_type, nullable) => {
            let key_str = builtin_type_str(key_type);
            let value_str = field_type_str(value_type);
            let base = format!("HashMap<{key_str}, {value_str}>");
            if *nullable {
                format!("Option<{base}>")
            } else {
                base
            }
        }
    }
}

fn builtin_type_str(bt: &ast::BuiltinType) -> String {
    match bt {
        ast::BuiltinType::Integer(it) => integer_type_str(it).to_string(),
        ast::BuiltinType::Float(ft) => match ft {
            ast::FloatType::F32 => "f32".to_string(),
            ast::FloatType::F64 => "f64".to_string(),
        },
        ast::BuiltinType::String => "String".to_string(),
        ast::BuiltinType::Bool => "bool".to_string(),
    }
}

fn integer_type_str(t: &ast::IntegerType) -> &'static str {
    match t {
        ast::IntegerType::I8 => "i8",
        ast::IntegerType::I16 => "i16",
        ast::IntegerType::I32 => "i32",
        ast::IntegerType::I64 => "i64",
        ast::IntegerType::U8 => "u8",
        ast::IntegerType::U16 => "u16",
        ast::IntegerType::U32 => "u32",
        ast::IntegerType::U64 => "u64",
    }
}

fn integer_value_str(v: &ast::IntegerValue) -> String {
    match v {
        ast::IntegerValue::I8(n) => n.to_string(),
        ast::IntegerValue::I16(n) => n.to_string(),
        ast::IntegerValue::I32(n) => n.to_string(),
        ast::IntegerValue::I64(n) => n.to_string(),
        ast::IntegerValue::U8(n) => n.to_string(),
        ast::IntegerValue::U16(n) => n.to_string(),
        ast::IntegerValue::U32(n) => n.to_string(),
        ast::IntegerValue::U64(n) => n.to_string(),
    }
}

/// Converts a string to PascalCase.
/// "type1" -> "Type1", "kiwiFruit" -> "KiwiFruit", "alpha_beta" -> "AlphaBeta"
fn to_pascal_case(s: &str) -> String {
    s.split('_')
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                None => String::new(),
                Some(c) => {
                    let mut s = c.to_uppercase().to_string();
                    s.push_str(chars.as_str());
                    s
                }
            }
        })
        .collect()
}

/// Converts a string to snake_case.
/// "alphaBeta" -> "alpha_beta", "alpha_beta" -> "alpha_beta"
fn to_snake_case(s: &str) -> String {
    let mut result = String::new();
    for (i, c) in s.chars().enumerate() {
        if c.is_uppercase() && i > 0 {
            result.push('_');
        }
        for lc in c.to_lowercase() {
            result.push(lc);
        }
    }
    result
}
