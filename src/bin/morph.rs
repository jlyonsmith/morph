use anyhow::{Context, bail};
use clap::Parser;
use duct::cmd;
use morph_tool::MorphAstBuilder;
use std::{
    fs::{self, File},
    io::{Write, stdout},
    path::PathBuf,
    process::exit,
};

#[derive(Parser)]
#[command(name = "morph", about = "Morph schema compiler")]
struct Cli {
    /// Input .morph file
    #[arg(value_name = "INPUT_FILE")]
    input_path: PathBuf,

    /// Output file path for the generated source code, or STDOUT if not provided
    #[arg(value_name = "OUTPUT_FILE", short = 'o', long)]
    output_path: Option<PathBuf>,

    /// Intermediate AST file path for debugging. Program will write the AST
    /// to this file in MessagePack format then exit.
    #[arg(value_name = "AST_FILE", short = 't', long)]
    ast_path: Option<PathBuf>,

    /// Output source code format (e.g. -f dart-json or -f rust-rmp)
    #[arg(value_name = "FORMAT", short = 'f', long)]
    format: Option<String>,
}

fn main() {
    match run() {
        Ok(code) => exit(code),
        Err(root_err) => {
            for err in root_err.chain() {
                eprintln!("error: {}", err);
            }
            exit(1);
        }
    }
}

fn run() -> anyhow::Result<i32> {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(err) => {
            // This prints the error message from clap
            eprintln!("{}", err);
            return Ok(0);
        }
    };

    // Parse the input string into an AST
    let ast_builder = MorphAstBuilder::new(cli.input_path);
    let ast = ast_builder.build()?;

    // If the user specified an AST output path, write the AST to that file and exit
    if let Some(ast_path) = cli.ast_path {
        let mut file = File::create(&ast_path).context(format!(
            "Could not create AST file '{}'",
            ast_path.to_string_lossy()
        ))?;

        rmp_serde::encode::write(&mut file, &ast)
            .context("Failed to serialize AST to MessagePack")?;
        return Ok(0);
    }

    let format = match cli.format {
        Some(s) => s,
        None => bail!("No output format specified"),
    };

    let cmd_expr = if std::env::var("MORPH_DEBUG").is_ok() {
        cmd!["cargo", "run", "--bin", &format!("morph-{}", format), "--"]
    } else {
        cmd![&format!("morph-{}", format)]
    };
    let ast_bytes = rmp_serde::to_vec(&ast).context("Failed to serialize AST to MessagePack")?;
    let output = cmd_expr
        .stdin_bytes(ast_bytes)
        .stdout_capture()
        .read()
        .with_context(|| format!("Failed to run AST formatter '{:?}'", cmd_expr))?;

    match cli.output_path {
        Some(path) => {
            fs::write(path, output)?;
        }
        None => {
            stdout().write(output.as_bytes())?;
        }
    };

    Ok(0)
}
