# Morph

[![coverage](https://shields.io/endpoint?url=https://raw.githubusercontent.com/jlyonsmith/morph/main/coverage.json)](https://github.com/jlyonsmith/morph/blob/main/coverage.json)
[![Crates.io](https://img.shields.io/crates/v/morph.svg)](https://crates.io/crates/morph)
[![Docs.rs](https://docs.rs/morph/badge.svg)](https://docs.rs/morph)

A cross-language schema compiler that generates type definitions and serialization code from a simple, declarative schema language.

Define your data types once in a `.morph` file, then generate idiomatic code for multiple target languages.

> This project is still in development. In particular, the schema language is not yet stable. Please feel free to contribute!

## Why?

There are several existing packing protocols with schema languages. In particular:

- [FlatBuffers](https://flatbuffers.dev/)
- [Cap'n Proto](https://capnproto.org/)
- [Protocol Buffers](https://protobuf.dev/) (also referred to as ProtoBuf)

 While they all generally have excellent programming language support, the schema languages for each are understandably tied to the underlying packing algorithms, and can be a little quirky. They also have some gaps. For example, it seems that most of these protocols existed before nullable types were standard across programming languages.

For my projects, I have found that the [MessagePack](https://msgpack.org/) protocol is actually the easiest packing protocol to work with, even if it is a little slower than the others. It's easy to integrate, perhaps because it is the closest to JSON, and JSON is still the most universal serialization format on the Internet.

You could say that Morph is a schema language for MessagePack, which it is. But I think, more importantly, Morph  easily supports other formats, such as JSON, YAML, TOML and [TOON](https://github.com/toon-format/toon). And, you could even use it to generate schemas for any of the above protocols.  So really, Morph is a schema definition language that easily supports any modern language and packing protocol.

Finally, I designed the AST for Morph to be a simple as possible, which makes it easy for Claude Code and other AI's to comprehend in a small number tokens.  This ought to make it easy to create generators for your programming language and packing protocol of choice.

## Schema Language

Morph schemas consist of a single `meta` section followed by any number of `enum` and `struct` declarations.

```
meta {
    format = 1,
}

enum fruit: i16 {
    apple = 1,
    orange = 2,
    kiwiFruit = 3,
    pear, // auto-incremented to 4
}

struct order {
    id: u64,
    name: string,
    quantity: i32,
    price: f64,
    fruit: fruit,
    tags: [string],
    metadata: {string: string},
    notes: string?,       // nullable
    items: [order; 10],   // fixed-length array
}
```

### Metadata

There must be at least one key to define the schema format being used:

| Key      | Values | Description |
|----------|--------|-------------|
| `format` | `1`    | This is the only supported schema value at present |

Otherwise, the `meta` section can contain any values that you like. You can use the `morph` crate to parse a `Schema` from a file and access the values easily.

### Types

| Category | Types |
|----------|-------|
| Integers | `i8`, `u8`, `i16`, `u16`, `i32`, `u32`, `i64`, `u64` |
| Floats | `f32`, `f64` |
| Other | `string`, `bool` |
| Arrays | `[T]` variable-length, `[T; N]` fixed-length |
| Maps | `{K: V}` where `K` is a builtin type |
| Nullable | Append `?` to any type |
| User-defined | Reference any declared enum or struct by name |

### Enums

Enums have an optional integer base type (defaults to `i32`). Variant values can be explicit or auto-incremented from the previous value.

```
enum color: u8 {
    red = 1,
    green = 2,
    blue = 3,
}
```

Integer literals support decimal, hex (`0xFF`), and binary (`0b1010`) notation.

### Comments

Single-line comments with `//`.

## Code Generators

| Format | Binary | Description |
|--------|--------|-------------|
| `rust-serde` | `morph-rust-serde` | Rust structs/enums with `Serialize`/`Deserialize` derives |
| `dart-mp` | `morph-dart-mp` | Dart classes/enums with MessagePack `toBytes`/`fromBytes` serialization |

### Rust Serde Output

- Derives `Debug`, `Clone`, `PartialEq`, `Serialize`, `Deserialize`
- Converts type names to `PascalCase` and field names to `snake_case`
- Adds `#[serde(rename = "...")]` when names are converted
- Maps arrays to `Vec<T>` or `[T; N]`, maps to `HashMap<K, V>`, nullable to `Option<T>`

### Dart MessagePack Output

- Generates classes with `final` fields and constructors with `required` named arguments
- Converts type names to `PascalCase` and field/variant names to `lowerCamelCase`
- Generates `toBytes()` and `static fromBytes()` methods using the [`messagepack`](https://pub.dev/packages/messagepack) package
- Handles nested structures, nullable types, lists, and maps
- All Dart integer types map to `int`, floats to `double`

## Usage

```bash
# Generate Rust code to stdout
morph schema.morph -f rust-serde

# Generate Dart code to a file
morph schema.morph -f dart-mp -o lib/generated.dart

# Dump the intermediate AST for debugging
morph schema.morph -t schema.ast
```

### CLI Options

```
morph <INPUT_FILE> [OPTIONS]

Arguments:
  <INPUT_FILE>           Input .morph file

Options:
  -o <OUTPUT_FILE>       Output file path (defaults to stdout)
  -f <FORMAT>            Output format (e.g. rust-serde, dart-mp)
  -t <AST_FILE>          Write intermediate AST in MessagePack format and exit
```

### Debug Mode

Set `MORPH_DEBUG=1` to invoke code generators via `cargo run` instead of looking for installed binaries on `PATH`:

```bash
MORPH_DEBUG=1 morph schema.morph -f rust-serde
```

## Architecture

Morph uses a multi-process pipeline. The main `morph` binary parses the schema and serializes the AST to MessagePack. It then pipes those bytes to a code generator binary (`morph-<format>`) via stdin, which writes generated source code to stdout.

```
.morph file ──► morph (parser + validator) ──► MessagePack AST ──► morph-<format> ──► source code
```

Code generators are standalone binaries that read a MessagePack-encoded `Schema` from stdin. This makes it straightforward to add new target languages without modifying the core parser.

## Building

Requires the Rust toolchain.

```bash
# Build all binaries
cargo build --release

# Install to ~/.cargo/bin
cargo install --path .
```

## Validation

The compiler checks for:

- Duplicate type names
- Duplicate field names within a struct
- Duplicate variant names within an enum
- References to undefined user-defined types
- Parse errors with line and column information
