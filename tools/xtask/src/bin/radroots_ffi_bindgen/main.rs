//! Source-owned native generation; no hand-edited generated bindings.
use anyhow::{Context, Result, ensure};
use camino::Utf8PathBuf;
use clap::{Parser, Subcommand, ValueEnum};
use std::fs;
use uniffi_bindgen::bindings::{GenerateOptions, TargetLanguage};

mod kotlin;
mod swift;

#[derive(Parser)]
#[command(version = "0.32.2+radroots-native-v1")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}
#[derive(Subcommand)]
enum Command {
    /// Generate from the actual multi-component native library.
    Generate {
        #[arg(long)]
        library: bool,
        source: Utf8PathBuf,
        #[arg(long, value_enum)]
        language: Language,
        #[arg(long)]
        out_dir: Utf8PathBuf,
        /// Stable unformatted output is required by the source-owned policy.
        #[arg(long)]
        no_format: bool,
    },
}
#[derive(Clone, Copy, ValueEnum)]
enum Language {
    Swift,
    Kotlin,
}

fn main() -> Result<()> {
    let Command::Generate {
        library: _,
        source,
        language,
        out_dir,
        no_format,
    } = Cli::parse().command;
    ensure!(no_format, "source-owned generation requires --no-format");
    ensure!(!out_dir.exists(), "output directory must be new");
    let staging = tempfile::tempdir().context("create generation staging")?;
    let stage = Utf8PathBuf::from_path_buf(staging.path().to_owned())
        .map_err(|_| anyhow::anyhow!("non-UTF8 generation staging"))?;
    uniffi_bindgen::bindings::generate(GenerateOptions {
        languages: vec![match language {
            Language::Swift => TargetLanguage::Swift,
            Language::Kotlin => TargetLanguage::Kotlin,
        }],
        source: source.clone(),
        out_dir: stage.clone(),
        format: false,
        ..Default::default()
    })?;
    {
        let components = uniffi_bindgen::library_mode::find_cis(
            &source,
            &uniffi_bindgen::EmptyCrateConfigSupplier,
        )?;
        for ci in components {
            if matches!(language, Language::Swift) {
                if ci.has_async_fns() {
                    let path = stage.join(format!("{}.swift", ci.namespace()));
                    let header =
                        fs::read_to_string(stage.join(format!("{}FFI.h", ci.namespace())))?;
                    let generated = swift::cancellation(&fs::read_to_string(&path)?, &header)?;
                    fs::write(path, generated)?;
                }
                continue;
            }
            let methods: Vec<_> = ci
                .object_definitions()
                .iter()
                .filter_map(|object| {
                    object
                        .methods()
                        .into_iter()
                        .find(|method| method.name() == "close")
                        .map(|method| (object.name().to_owned(), method.is_async()))
                })
                .collect();
            if methods.is_empty() {
                continue;
            }
            // This producer has no foreign renaming/package configuration. Fail
            // closed if its source-owned canonical package layout ever changes.
            let path = stage.join(format!("uniffi/{0}/{0}.kt", ci.namespace()));
            let original = fs::read_to_string(&path).context("read Kotlin component")?;
            let generated = kotlin::preserve_close(&original, &methods)?;
            fs::write(path, generated)?;
        }
    }
    copy_tree(&stage, &out_dir)?;
    Ok(())
}

fn copy_tree(source: &camino::Utf8Path, output: &camino::Utf8Path) -> Result<()> {
    fs::create_dir(output)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let destination = output.join(
            entry
                .file_name()
                .to_str()
                .context("non-UTF8 generated path")?,
        );
        let kind = entry.file_type()?;
        if kind.is_dir() {
            let path = Utf8PathBuf::from_path_buf(entry.path())
                .map_err(|_| anyhow::anyhow!("non-UTF8 generated path"))?;
            copy_tree(&path, &destination)?;
        } else {
            ensure!(kind.is_file(), "unexpected generated entry type");
            fs::copy(entry.path(), destination)?;
        }
    }
    Ok(())
}
