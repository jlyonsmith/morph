//! Morph Dart/MessagePacker generator.  All type categories are handled:
//!
//! - Primitive types: p.packInt() / u.unpackInt()! etc.
//! - Nullable primitives: null check with p.packNull() fallback / u.unpackInt() (returns nullable)
//! - Enums: _pack(p) packs the int value / _unpack(u) does firstWhere lookup
//! - Nullable enums: null check + _pack / _unpackNullable checks unpackInt() for null
//! - Lists: packListLength + element loop / List.generate(u.unpackListLength(), ...)
//! - Nullable lists: presence marker packBool(true) / u.unpackBool() == null ? null : ...
//! - Maps: packMapLength + entry loop / Map.fromEntries(List.generate(u.unpackMapLength(), ...))
//! - Nested structs: _pack(p) / Type._unpack(u) — correctly recursive
//! - Nullable structs: presence marker packBool(true) + _pack / _unpackNullable checks unpackBool() for null
//!
//! Here's a summary of the serialization protocol:
//!
//! ┌─────────────────────┬─────────────────────────────────────────┬─────────────────────────────┐
//! │        Type         │               Pack format               │           Unpack            │
//! ├─────────────────────┼─────────────────────────────────────────┼─────────────────────────────┤
//! │ Primitives          │ Direct packXXX                          │ unpackXXX()!                │
//! ├─────────────────────┼─────────────────────────────────────────┼─────────────────────────────┤
//! │ Nullable primitives │ packNull or packXXX                     │ unpackXXX() (returns T?)    │
//! ├─────────────────────┼─────────────────────────────────────────┼─────────────────────────────┤
//! │ Enums               │ packInt(value)                          │ firstWhere on unpackInt()!  │
//! ├─────────────────────┼─────────────────────────────────────────┼─────────────────────────────┤
//! │ Nullable enums      │ packNull or packInt(value)              │ Check unpackInt() for null  │
//! ├─────────────────────┼─────────────────────────────────────────┼─────────────────────────────┤
//! │ Structs             │ Sequential field packing                │ Sequential field unpacking  │
//! ├─────────────────────┼─────────────────────────────────────────┼─────────────────────────────┤
//! │ Nullable structs    │ packNull or packBool(true) + fields     │ Check unpackBool() for null │
//! ├─────────────────────┼─────────────────────────────────────────┼─────────────────────────────┤
//! │ Nullable lists/maps │ packNull or packBool(true) + collection │ Check unpackBool() for null │
//! └─────────────────────┴─────────────────────────────────────────┴─────────────────────────────┘
//!
use anyhow::Context;
use morph_tool::ast;
use std::collections::HashSet;
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

    let enum_names: HashSet<&str> = schema
        .declarations
        .iter()
        .filter_map(|d| match d {
            ast::Declaration::Enum { ident, .. } => Some(ident.as_str()),
            _ => None,
        })
        .collect();

    writeln!(out, "import 'dart:typed_data';").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "import 'package:messagepack/messagepack.dart';").unwrap();

    for decl in &schema.declarations {
        writeln!(out).unwrap();
        match decl {
            ast::Declaration::Enum {
                ident,
                base_type,
                variants,
            } => generate_enum(&mut out, ident, base_type, variants),
            ast::Declaration::Struct { ident, fields } => {
                generate_struct(&mut out, ident, fields, &enum_names)
            }
        }
    }

    out
}

fn generate_enum(
    out: &mut String,
    ident: &str,
    _base_type: &ast::IntegerType,
    variants: &[(String, Option<ast::IntegerValue>)],
) {
    let dart_name = to_pascal_case(ident);

    writeln!(out, "enum {dart_name} {{").unwrap();

    let mut next_value: i64 = 0;
    for (i, (variant_name, value)) in variants.iter().enumerate() {
        let dart_variant = to_lower_camel_case(variant_name);
        let trailing = if i < variants.len() - 1 { "," } else { ";" };
        let actual_value = match value {
            Some(v) => {
                let val = integer_value_to_i64(v);
                next_value = val + 1;
                val
            }
            None => {
                let val = next_value;
                next_value += 1;
                val
            }
        };
        writeln!(out, "  {dart_variant}({actual_value}){trailing}").unwrap();
    }

    writeln!(out).unwrap();
    writeln!(out, "  final int value;").unwrap();
    writeln!(out, "  const {dart_name}(this.value);").unwrap();

    // toBytes
    writeln!(out).unwrap();
    writeln!(out, "  Uint8List toBytes() {{").unwrap();
    writeln!(out, "    final p = Packer();").unwrap();
    writeln!(out, "    _pack(p);").unwrap();
    writeln!(out, "    return p.takeBytes();").unwrap();
    writeln!(out, "  }}").unwrap();

    // fromBytes
    writeln!(out).unwrap();
    writeln!(out, "  static {dart_name} fromBytes(Uint8List bytes) {{").unwrap();
    writeln!(out, "    return _unpack(Unpacker(bytes));").unwrap();
    writeln!(out, "  }}").unwrap();

    // _pack
    writeln!(out).unwrap();
    writeln!(out, "  void _pack(Packer p) {{").unwrap();
    writeln!(out, "    p.packInt(value);").unwrap();
    writeln!(out, "  }}").unwrap();

    // _unpack
    writeln!(out).unwrap();
    writeln!(out, "  static {dart_name} _unpack(Unpacker u) {{").unwrap();
    writeln!(
        out,
        "    return values.firstWhere((e) => e.value == u.unpackInt()!);"
    )
    .unwrap();
    writeln!(out, "  }}").unwrap();

    // _unpackNullable
    writeln!(out).unwrap();
    writeln!(out, "  static {dart_name}? _unpackNullable(Unpacker u) {{").unwrap();
    writeln!(out, "    final v = u.unpackInt();").unwrap();
    writeln!(
        out,
        "    return v == null ? null : values.firstWhere((e) => e.value == v);"
    )
    .unwrap();
    writeln!(out, "  }}").unwrap();

    writeln!(out, "}}").unwrap();
}

fn generate_struct(
    out: &mut String,
    ident: &str,
    fields: &[(String, ast::FieldType)],
    enum_names: &HashSet<&str>,
) {
    let dart_name = to_pascal_case(ident);

    writeln!(out, "class {dart_name} {{").unwrap();

    // Fields
    for (field_name, field_type) in fields {
        let dart_field = to_lower_camel_case(field_name);
        writeln!(out, "  final {} {dart_field};", field_type_str(field_type)).unwrap();
    }

    // Constructor
    writeln!(out).unwrap();
    writeln!(out, "  {dart_name}({{").unwrap();
    for (field_name, field_type) in fields {
        let dart_field = to_lower_camel_case(field_name);
        if is_nullable(field_type) {
            writeln!(out, "    this.{dart_field},").unwrap();
        } else {
            writeln!(out, "    required this.{dart_field},").unwrap();
        }
    }
    writeln!(out, "  }});").unwrap();

    // toBytes
    writeln!(out).unwrap();
    writeln!(out, "  Uint8List toBytes() {{").unwrap();
    writeln!(out, "    final p = Packer();").unwrap();
    writeln!(out, "    _pack(p);").unwrap();
    writeln!(out, "    return p.takeBytes();").unwrap();
    writeln!(out, "  }}").unwrap();

    // fromBytes
    writeln!(out).unwrap();
    writeln!(out, "  static {dart_name} fromBytes(Uint8List bytes) {{").unwrap();
    writeln!(out, "    return _unpack(Unpacker(bytes));").unwrap();
    writeln!(out, "  }}").unwrap();

    // _pack
    writeln!(out).unwrap();
    writeln!(out, "  void _pack(Packer p) {{").unwrap();
    for (field_name, field_type) in fields {
        let dart_field = to_lower_camel_case(field_name);
        generate_pack_field(out, &dart_field, field_type, "    ", enum_names, 0);
    }
    writeln!(out, "  }}").unwrap();

    // _unpack
    writeln!(out).unwrap();
    writeln!(out, "  static {dart_name} _unpack(Unpacker u) {{").unwrap();
    for (field_name, field_type) in fields {
        let dart_field = to_lower_camel_case(field_name);
        let expr = generate_unpack_expr(field_type, enum_names);
        writeln!(out, "    final {dart_field} = {expr};").unwrap();
    }
    writeln!(out, "    return {dart_name}(").unwrap();
    for (field_name, _) in fields {
        let dart_field = to_lower_camel_case(field_name);
        writeln!(out, "      {dart_field}: {dart_field},").unwrap();
    }
    writeln!(out, "    );").unwrap();
    writeln!(out, "  }}").unwrap();

    // _unpackNullable
    writeln!(out).unwrap();
    writeln!(out, "  static {dart_name}? _unpackNullable(Unpacker u) {{").unwrap();
    writeln!(out, "    if (u.unpackBool() == null) return null;").unwrap();
    writeln!(out, "    return _unpack(u);").unwrap();
    writeln!(out, "  }}").unwrap();

    writeln!(out, "}}").unwrap();
}

fn generate_pack_field(
    out: &mut String,
    expr: &str,
    ft: &ast::FieldType,
    indent: &str,
    enum_names: &HashSet<&str>,
    depth: usize,
) {
    match ft {
        ast::FieldType::Builtin(bt, nullable) => {
            let method = builtin_pack_method(bt);
            if *nullable {
                writeln!(out, "{indent}if ({expr} != null) {{").unwrap();
                writeln!(out, "{indent}  p.{method}({expr}!);").unwrap();
                writeln!(out, "{indent}}} else {{").unwrap();
                writeln!(out, "{indent}  p.packNull();").unwrap();
                writeln!(out, "{indent}}}").unwrap();
            } else {
                writeln!(out, "{indent}p.{method}({expr});").unwrap();
            }
        }
        ast::FieldType::UserDefined(name, nullable) => {
            let is_enum = enum_names.contains(name.as_str());
            if *nullable {
                writeln!(out, "{indent}if ({expr} != null) {{").unwrap();
                if !is_enum {
                    writeln!(out, "{indent}  p.packBool(true);").unwrap();
                }
                writeln!(out, "{indent}  {expr}!._pack(p);").unwrap();
                writeln!(out, "{indent}}} else {{").unwrap();
                writeln!(out, "{indent}  p.packNull();").unwrap();
                writeln!(out, "{indent}}}").unwrap();
            } else {
                writeln!(out, "{indent}{expr}._pack(p);").unwrap();
            }
        }
        ast::FieldType::Array(inner, _, nullable) => {
            let var = format!("e{depth}");
            let src = if *nullable {
                format!("{expr}!")
            } else {
                expr.to_string()
            };
            if *nullable {
                writeln!(out, "{indent}if ({expr} != null) {{").unwrap();
                writeln!(out, "{indent}  p.packBool(true);").unwrap();
                writeln!(out, "{indent}  p.packListLength({src}.length);").unwrap();
                writeln!(out, "{indent}  for (final {var} in {src}) {{").unwrap();
                generate_pack_field(
                    out,
                    &var,
                    inner,
                    &format!("{indent}    "),
                    enum_names,
                    depth + 1,
                );
                writeln!(out, "{indent}  }}").unwrap();
                writeln!(out, "{indent}}} else {{").unwrap();
                writeln!(out, "{indent}  p.packNull();").unwrap();
                writeln!(out, "{indent}}}").unwrap();
            } else {
                writeln!(out, "{indent}p.packListLength({expr}.length);").unwrap();
                writeln!(out, "{indent}for (final {var} in {expr}) {{").unwrap();
                generate_pack_field(
                    out,
                    &var,
                    inner,
                    &format!("{indent}  "),
                    enum_names,
                    depth + 1,
                );
                writeln!(out, "{indent}}}").unwrap();
            }
        }
        ast::FieldType::Map(key_type, value_type, nullable) => {
            let var = format!("e{depth}");
            let key_method = builtin_pack_method(key_type);
            let src = if *nullable {
                format!("{expr}!")
            } else {
                expr.to_string()
            };
            if *nullable {
                writeln!(out, "{indent}if ({expr} != null) {{").unwrap();
                writeln!(out, "{indent}  p.packBool(true);").unwrap();
                writeln!(out, "{indent}  p.packMapLength({src}.length);").unwrap();
                writeln!(out, "{indent}  for (final {var} in {src}.entries) {{").unwrap();
                writeln!(out, "{indent}    p.{key_method}({var}.key);").unwrap();
                generate_pack_field(
                    out,
                    &format!("{var}.value"),
                    value_type,
                    &format!("{indent}    "),
                    enum_names,
                    depth + 1,
                );
                writeln!(out, "{indent}  }}").unwrap();
                writeln!(out, "{indent}}} else {{").unwrap();
                writeln!(out, "{indent}  p.packNull();").unwrap();
                writeln!(out, "{indent}}}").unwrap();
            } else {
                writeln!(out, "{indent}p.packMapLength({expr}.length);").unwrap();
                writeln!(out, "{indent}for (final {var} in {expr}.entries) {{").unwrap();
                writeln!(out, "{indent}  p.{key_method}({var}.key);").unwrap();
                generate_pack_field(
                    out,
                    &format!("{var}.value"),
                    value_type,
                    &format!("{indent}  "),
                    enum_names,
                    depth + 1,
                );
                writeln!(out, "{indent}}}").unwrap();
            }
        }
    }
}

fn generate_unpack_expr(ft: &ast::FieldType, enum_names: &HashSet<&str>) -> String {
    match ft {
        ast::FieldType::Builtin(bt, nullable) => {
            let method = builtin_unpack_method(bt);
            if *nullable {
                format!("u.{method}()")
            } else {
                format!("u.{method}()!")
            }
        }
        ast::FieldType::UserDefined(name, nullable) => {
            let dart_name = to_pascal_case(name);
            if *nullable {
                format!("{dart_name}._unpackNullable(u)")
            } else {
                format!("{dart_name}._unpack(u)")
            }
        }
        ast::FieldType::Array(inner, _, nullable) => {
            let inner_expr = generate_unpack_expr(inner, enum_names);
            let base = format!("List.generate(u.unpackListLength(), (_) => {inner_expr})");
            if *nullable {
                format!("u.unpackBool() == null ? null : {base}")
            } else {
                base
            }
        }
        ast::FieldType::Map(key_type, value_type, nullable) => {
            let key_method = builtin_unpack_method(key_type);
            let value_expr = generate_unpack_expr(value_type, enum_names);
            let base = format!(
                "Map.fromEntries(List.generate(u.unpackMapLength(), (_) => MapEntry(u.{key_method}()!, {value_expr})))"
            );
            if *nullable {
                format!("u.unpackBool() == null ? null : {base}")
            } else {
                base
            }
        }
    }
}

fn is_nullable(ft: &ast::FieldType) -> bool {
    match ft {
        ast::FieldType::Builtin(_, nullable) => *nullable,
        ast::FieldType::UserDefined(_, nullable) => *nullable,
        ast::FieldType::Array(_, _, nullable) => *nullable,
        ast::FieldType::Map(_, _, nullable) => *nullable,
    }
}

fn field_type_str(ft: &ast::FieldType) -> String {
    match ft {
        ast::FieldType::Builtin(bt, nullable) => {
            let base = builtin_type_str(bt);
            if *nullable { format!("{base}?") } else { base }
        }
        ast::FieldType::UserDefined(name, nullable) => {
            let dart_name = to_pascal_case(name);
            if *nullable {
                format!("{dart_name}?")
            } else {
                dart_name
            }
        }
        ast::FieldType::Array(inner, _length, nullable) => {
            let inner_str = field_type_str(inner);
            let base = format!("List<{inner_str}>");
            if *nullable { format!("{base}?") } else { base }
        }
        ast::FieldType::Map(key_type, value_type, nullable) => {
            let key_str = builtin_type_str(key_type);
            let value_str = field_type_str(value_type);
            let base = format!("Map<{key_str}, {value_str}>");
            if *nullable { format!("{base}?") } else { base }
        }
    }
}

fn builtin_type_str(bt: &ast::BuiltinType) -> String {
    match bt {
        ast::BuiltinType::Integer(_) => "int".to_string(),
        ast::BuiltinType::Float(_) => "double".to_string(),
        ast::BuiltinType::String => "String".to_string(),
        ast::BuiltinType::Bool => "bool".to_string(),
    }
}

fn builtin_pack_method(bt: &ast::BuiltinType) -> &'static str {
    match bt {
        ast::BuiltinType::Integer(_) => "packInt",
        ast::BuiltinType::Float(_) => "packDouble",
        ast::BuiltinType::String => "packString",
        ast::BuiltinType::Bool => "packBool",
    }
}

fn builtin_unpack_method(bt: &ast::BuiltinType) -> &'static str {
    match bt {
        ast::BuiltinType::Integer(_) => "unpackInt",
        ast::BuiltinType::Float(_) => "unpackDouble",
        ast::BuiltinType::String => "unpackString",
        ast::BuiltinType::Bool => "unpackBool",
    }
}

fn integer_value_to_i64(v: &ast::IntegerValue) -> i64 {
    match v {
        ast::IntegerValue::I8(n) => *n as i64,
        ast::IntegerValue::I16(n) => *n as i64,
        ast::IntegerValue::I32(n) => *n as i64,
        ast::IntegerValue::I64(n) => *n,
        ast::IntegerValue::U8(n) => *n as i64,
        ast::IntegerValue::U16(n) => *n as i64,
        ast::IntegerValue::U32(n) => *n as i64,
        ast::IntegerValue::U64(n) => *n as i64,
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

/// Converts a string to lowerCamelCase.
/// "alpha_beta" -> "alphaBeta", "AlphaBeta" -> "alphaBeta"
fn to_lower_camel_case(s: &str) -> String {
    let parts: Vec<&str> = s.split('_').collect();
    let mut result = String::new();

    for (i, part) in parts.iter().enumerate() {
        let mut chars = part.chars();
        match chars.next() {
            None => {}
            Some(c) => {
                if i == 0 {
                    for lc in c.to_lowercase() {
                        result.push(lc);
                    }
                } else {
                    for uc in c.to_uppercase() {
                        result.push(uc);
                    }
                }
                result.push_str(chars.as_str());
            }
        }
    }

    result
}
