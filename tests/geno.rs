use duct::cmd;
use std::fs;
use tempfile::TempDir;

#[test]
fn parse_example_and_write_ast() {
    let dir = TempDir::new().unwrap();
    let ast_path = dir.path().join("example.ast");
    let ast_str = ast_path.to_str().unwrap();

    let output = cmd![
        "cargo",
        "run",
        "--bin",
        "geno",
        "--",
        "examples/example.geno",
        "-t",
        ast_str
    ]
    .stdout_capture()
    .stderr_capture()
    .unchecked()
    .run()
    .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(ast_path.exists());
    let ast_bytes = fs::read(&ast_path).unwrap();
    assert!(!ast_bytes.is_empty());

    // Verify the AST deserializes back to a valid Schema
    let schema: geno::ast::Schema = rmp_serde::from_slice(&ast_bytes).unwrap();
    assert!(!schema.declarations.is_empty());
}

#[test]
fn generate_rust_serde() {
    let output = cmd![
        "cargo",
        "run",
        "--bin",
        "geno",
        "--",
        "examples/example.geno",
        "-f",
        "rust-serde"
    ]
    .env("GENO_DEBUG", "1")
    .stdout_capture()
    .stderr_capture()
    .unchecked()
    .run()
    .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("pub enum"));
    assert!(stdout.contains("pub struct"));
    assert!(stdout.contains("Serialize"));
    assert!(stdout.contains("Deserialize"));
}

#[test]
fn generate_dart_mp() {
    let output = cmd![
        "cargo",
        "run",
        "--bin",
        "geno",
        "--",
        "examples/example.geno",
        "-f",
        "dart-mp"
    ]
    .env("GENO_DEBUG", "1")
    .stdout_capture()
    .stderr_capture()
    .unchecked()
    .run()
    .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("enum "));
    assert!(stdout.contains("class "));
    assert!(stdout.contains("toBytes"));
    assert!(stdout.contains("fromBytes"));
    assert!(stdout.contains("import 'package:messagepack/messagepack.dart'"));
}

#[test]
fn generate_to_output_file() {
    let dir = TempDir::new().unwrap();
    let out_path = dir.path().join("output.rs");
    let out_str = out_path.to_str().unwrap();

    let output = cmd![
        "cargo",
        "run",
        "--bin",
        "geno",
        "--",
        "examples/example.geno",
        "-f",
        "rust-serde",
        "-o",
        out_str
    ]
    .env("GENO_DEBUG", "1")
    .stdout_capture()
    .stderr_capture()
    .unchecked()
    .run()
    .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let content = fs::read_to_string(&out_path).unwrap();
    assert!(content.contains("pub struct"));
}

#[test]
fn missing_input_file() {
    let output = cmd![
        "cargo",
        "run",
        "--bin",
        "geno",
        "--",
        "nonexistent.geno",
        "-f",
        "rust-serde"
    ]
    .env("GENO_DEBUG", "1")
    .stdout_capture()
    .stderr_capture()
    .unchecked()
    .run()
    .unwrap();

    assert!(!output.status.success());
}

#[test]
fn no_format_specified() {
    let output = cmd![
        "cargo",
        "run",
        "--bin",
        "geno",
        "--",
        "examples/example.geno"
    ]
    .stdout_capture()
    .stderr_capture()
    .unchecked()
    .run()
    .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("No output format specified"));
}
