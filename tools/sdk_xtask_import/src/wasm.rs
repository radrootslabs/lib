use std::{
    collections::BTreeSet,
    env,
    ffi::OsStr,
    fs, io,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use crate::{
    check::check_wasm_package_surface,
    fs::workspace_root,
    package_matrix::{WasmPackageSpec, validate_package_matrix, wasm_package_specs},
    wasm_declarations::write_declaration_files,
};

pub(crate) const WASM_TARGET: &str = "wasm32-unknown-unknown";
const WASM_C_COMPILER_ENV: &str = "CC_wasm32_unknown_unknown";
const WASM_CFLAGS_ENV: &str = "CFLAGS_wasm32_unknown_unknown";
const WASM_C_TARGET_ARG: &str = "--target=wasm32-unknown-unknown";

pub fn generate(args: &[String]) -> Result<(), String> {
    validate_package_matrix()?;
    let specs = selected_specs(args)?;
    let root = workspace_root()?;
    let toolchain = resolve_wasm_toolchain(&root)?;
    println!(
        "using Rust toolchain {} for {}: rustc={}, cargo={}",
        toolchain.channel(),
        WASM_TARGET,
        toolchain.rustc().display(),
        toolchain.cargo().display()
    );
    let wasm_c_compiler = if specs
        .iter()
        .any(|spec| wasm_package_requires_c_compiler(*spec))
    {
        let compiler = resolve_wasm_c_compiler()?;
        println!(
            "using WASM C compiler for {}: {}",
            WASM_TARGET,
            compiler.path.display()
        );
        Some(compiler)
    } else {
        None
    };
    for spec in specs {
        let dist_dir = root.join(spec.package_dir).join("dist");
        if dist_dir.exists() {
            fs::remove_dir_all(&dist_dir)
                .map_err(|error| format!("failed to remove {}: {error}", dist_dir.display()))?;
        }
        let mut command = Command::new(&toolchain.wasm_pack);
        command.current_dir(&root);
        for arg in wasm_pack_args(spec) {
            command.arg(arg);
        }
        toolchain.apply_to_command(&mut command);
        if let Some(compiler) = wasm_c_compiler.as_ref()
            && wasm_package_requires_c_compiler(spec)
        {
            compiler.apply_to_command(&mut command);
        }
        let status = command.status().map_err(|error| {
            format!(
                "failed to start wasm-pack for {} while generating {}: {error}",
                spec.key, spec.package_name
            )
        })?;
        if !status.success() {
            return Err(format!(
                "wasm-pack failed for {} while generating {} with status {status}; rerun `cargo xtask generate wasm --package {}` after fixing the wasm toolchain",
                spec.key, spec.package_name, spec.key
            ));
        }
        remove_wasm_pack_gitignore(&dist_dir, spec)?;
        write_declaration_files(&root, spec)?;
        check_wasm_package_surface(&root, spec)?;
        println!("generated wasm package {}", spec.package_name);
    }
    Ok(())
}

struct WasmToolchain {
    wasm_pack: PathBuf,
    rust: ResolvedRustToolchain,
}

impl WasmToolchain {
    fn apply_to_command(&self, command: &mut Command) {
        self.rust.apply_to_command(command);
    }

    fn rustc(&self) -> &Path {
        &self.rust.rustc
    }

    fn cargo(&self) -> &Path {
        &self.rust.cargo
    }

    fn channel(&self) -> &str {
        &self.rust.channel
    }
}

pub(crate) struct ResolvedRustToolchain {
    pub(crate) channel: String,
    pub(crate) rustc: PathBuf,
    pub(crate) cargo: PathBuf,
    pub(crate) bin_dir: PathBuf,
}

impl ResolvedRustToolchain {
    pub(crate) fn apply_to_command(&self, command: &mut Command) {
        prepend_path(command, &self.bin_dir);
        command.env("RUSTC", &self.rustc);
        command.env("CARGO", &self.cargo);
        command.env("RUSTUP_TOOLCHAIN", &self.channel);
    }
}

fn resolve_wasm_toolchain(root: &Path) -> Result<WasmToolchain, String> {
    let wasm_pack = resolve_required_path_tool("wasm-pack")?;
    let rust = resolve_rust_toolchain(root)?;
    Ok(WasmToolchain { wasm_pack, rust })
}

pub(crate) fn resolve_rust_toolchain(root: &Path) -> Result<ResolvedRustToolchain, String> {
    let toolchain_path = root.join("rust-toolchain.toml");
    let channel = read_rust_toolchain_channel(&toolchain_path)?;
    let rustc = rustup_tool_for_channel("rustc", &channel)?;
    let cargo = rustup_tool_for_channel("cargo", &channel)?;
    let rustc_bin = rustc.parent().ok_or_else(|| {
        format!(
            "rustup resolved rustc for toolchain {channel} without a parent path: {}",
            rustc.display()
        )
    })?;
    let cargo_bin = cargo.parent().ok_or_else(|| {
        format!(
            "rustup resolved cargo for toolchain {channel} without a parent path: {}",
            cargo.display()
        )
    })?;
    if rustc_bin != cargo_bin {
        return Err(format!(
            "rustup resolved mismatched Rust tool paths for toolchain {channel}: rustc={}, cargo={}",
            rustc.display(),
            cargo.display()
        ));
    }
    let bin_dir = rustc_bin.to_path_buf();
    ensure_wasm_target_installed(&channel)?;
    Ok(ResolvedRustToolchain {
        channel,
        rustc,
        cargo,
        bin_dir,
    })
}

fn wasm_pack_args(spec: WasmPackageSpec) -> Vec<&'static str> {
    vec![
        "build",
        spec.crate_dir,
        "--release",
        "--target",
        "web",
        "--out-dir",
        spec.out_dir,
        "--out-name",
        spec.out_name,
        "--no-pack",
    ]
}

fn remove_wasm_pack_gitignore(dist_dir: &Path, spec: WasmPackageSpec) -> Result<(), String> {
    let ignore_path = dist_dir.join(".gitignore");
    let contents = match fs::read_to_string(&ignore_path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(format!(
                "failed to read wasm-pack ignore file for {}: {}: {error}",
                spec.package_name,
                ignore_path.display()
            ));
        }
    };
    if contents.trim() != "*" {
        return Err(format!(
            "unexpected wasm-pack ignore file for {}: {}; refusing to remove it",
            spec.package_name,
            ignore_path.display()
        ));
    }
    fs::remove_file(&ignore_path).map_err(|error| {
        format!(
            "failed to remove wasm-pack ignore file for {}: {}: {error}",
            spec.package_name,
            ignore_path.display()
        )
    })
}

fn selected_specs(args: &[String]) -> Result<Vec<WasmPackageSpec>, String> {
    match args {
        [] => Ok(wasm_package_specs().to_vec()),
        [flag, key] if flag == "--package" => wasm_package_specs()
            .iter()
            .copied()
            .find(|spec| spec.key == key)
            .map(|spec| vec![spec])
            .ok_or_else(|| format!("unknown wasm package: {key}")),
        _ => Err("usage: cargo xtask generate wasm [--package <key>]".to_owned()),
    }
}

fn wasm_package_requires_c_compiler(spec: WasmPackageSpec) -> bool {
    spec.key == "event_codec"
}

fn resolve_required_path_tool(name: &str) -> Result<PathBuf, String> {
    let path = env::var_os("PATH").ok_or_else(|| {
        format!("missing {name}: PATH is not set; install {name} and expose it on PATH")
    })?;
    resolve_path_tool_from_path(name, &path)
}

fn resolve_path_tool_from_path(name: &str, path: &std::ffi::OsStr) -> Result<PathBuf, String> {
    let matches = executable_matches(name, path);
    match matches.as_slice() {
        [] => Err(format!(
            "missing {name}: install {name} and rerun `cargo xtask generate wasm`"
        )),
        [tool] => Ok(tool.clone()),
        _ => Err(format!(
            "ambiguous {name}: found {}; remove duplicate {name} entries from PATH before running `cargo xtask generate wasm`",
            matches
                .iter()
                .map(|path| path.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        )),
    }
}

fn executable_matches(name: &str, path: &std::ffi::OsStr) -> Vec<PathBuf> {
    let mut seen = BTreeSet::new();
    let mut matches = Vec::new();
    for dir in env::split_paths(path) {
        let candidate = dir.join(name);
        if !is_executable_file(&candidate) {
            continue;
        }
        let key = fs::canonicalize(&candidate).unwrap_or_else(|_| candidate.clone());
        if seen.insert(key) {
            matches.push(candidate);
        }
    }
    matches
}

fn is_executable_file(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        path.metadata()
            .map(|metadata| metadata.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
    }
    #[cfg(not(unix))]
    {
        true
    }
}

fn read_rust_toolchain_channel(path: &Path) -> Result<String, String> {
    let contents = fs::read_to_string(path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    rust_toolchain_channel_from(&contents)
}

fn rust_toolchain_channel_from(contents: &str) -> Result<String, String> {
    let value = contents
        .parse::<toml::Value>()
        .map_err(|error| format!("failed to parse rust-toolchain.toml: {error}"))?;
    let channel = value
        .get("toolchain")
        .and_then(|toolchain| toolchain.get("channel"))
        .and_then(toml::Value::as_str)
        .ok_or_else(|| "rust-toolchain.toml must define toolchain.channel".to_owned())?
        .trim();
    if channel.is_empty() {
        return Err("rust-toolchain.toml toolchain.channel must not be empty".to_owned());
    }
    Ok(channel.to_owned())
}

fn rustup_tool_for_channel(name: &str, channel: &str) -> Result<PathBuf, String> {
    let output = Command::new("rustup")
        .arg("which")
        .arg("--toolchain")
        .arg(channel)
        .arg(name)
        .output()
        .map_err(|error| {
            format!(
                "failed to resolve {name} for Rust toolchain {channel} with rustup: {error}; install rustup toolchain {channel}"
            )
        })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "failed to resolve {name} for Rust toolchain {channel}: {}; run `rustup toolchain install {channel}`",
            stderr.trim()
        ));
    }
    let path = String::from_utf8(output.stdout)
        .map_err(|error| format!("rustup emitted non-UTF-8 {name} path: {error}"))?;
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err(format!(
            "rustup returned an empty {name} path for Rust toolchain {channel}"
        ));
    }
    Ok(PathBuf::from(trimmed))
}

fn ensure_wasm_target_installed(channel: &str) -> Result<(), String> {
    let output = Command::new("rustup")
        .args(rustup_target_list_args(channel))
        .output()
        .map_err(|error| {
            format!(
                "failed to verify {WASM_TARGET} target for Rust toolchain {channel} with rustup: {error}; install rustup toolchain {channel}"
            )
        })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "failed to verify {WASM_TARGET} target for Rust toolchain {channel}: {}; run `rustup target add {WASM_TARGET} --toolchain {channel}`",
            stderr.trim()
        ));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    validate_target_list(&stdout, WASM_TARGET, channel)
}

fn rustup_target_list_args(channel: &str) -> [&str; 5] {
    ["target", "list", "--installed", "--toolchain", channel]
}

fn validate_target_list(output: &str, target: &str, channel: &str) -> Result<(), String> {
    if target_list_contains(output, target) {
        Ok(())
    } else {
        Err(format!(
            "missing Rust target {target} for Rust toolchain {channel}: run `rustup target add {target} --toolchain {channel}`"
        ))
    }
}

fn target_list_contains(output: &str, target: &str) -> bool {
    output.lines().any(|line| line.trim() == target)
}

#[derive(Debug)]
struct WasmCCompiler {
    path: PathBuf,
    cflags: ResolvedWasmCFlags,
}

impl WasmCCompiler {
    fn apply_to_command(&self, command: &mut Command) {
        command.env(WASM_C_COMPILER_ENV, &self.path);
        command.env(WASM_CFLAGS_ENV, &self.cflags.value);
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ResolvedWasmCFlags {
    value: String,
    args: Vec<String>,
}

fn resolve_wasm_c_compiler() -> Result<WasmCCompiler, String> {
    let path = env::var_os("PATH").unwrap_or_default();
    let explicit = env::var_os(WASM_C_COMPILER_ENV);
    let cflags = resolve_wasm_cflags(env::var_os(WASM_CFLAGS_ENV).as_deref())?;
    let candidates = wasm_c_compiler_candidates(&path);
    resolve_wasm_c_compiler_with(
        explicit.as_deref(),
        &path,
        candidates,
        cflags,
        verify_wasm_c_compiler,
    )
}

fn resolve_wasm_c_compiler_with<F>(
    explicit: Option<&OsStr>,
    path: &OsStr,
    candidates: Vec<PathBuf>,
    cflags: ResolvedWasmCFlags,
    verify: F,
) -> Result<WasmCCompiler, String>
where
    F: Fn(&Path, &ResolvedWasmCFlags) -> Result<(), String>,
{
    if let Some(explicit) = explicit {
        let compiler = resolve_explicit_wasm_c_compiler(explicit, path)?;
        verify(&compiler, &cflags).map_err(|error| {
            format!(
                "{WASM_C_COMPILER_ENV} does not name a WASM-capable C compiler: {}; {error}",
                compiler.display()
            )
        })?;
        return Ok(WasmCCompiler {
            path: compiler,
            cflags,
        });
    }

    let mut checked = Vec::new();
    for candidate in candidates {
        if !is_executable_file(&candidate) {
            continue;
        }
        checked.push(candidate.display().to_string());
        if verify(&candidate, &cflags).is_ok() {
            return Ok(WasmCCompiler {
                path: candidate,
                cflags,
            });
        }
    }
    let checked = if checked.is_empty() {
        "none".to_owned()
    } else {
        checked.join(", ")
    };
    Err(format!(
        "missing suitable WASM C compiler for {WASM_TARGET}: set {WASM_C_COMPILER_ENV} to a clang-style compiler that accepts `{WASM_C_TARGET_ARG}`; checked candidates: {checked}"
    ))
}

fn resolve_wasm_cflags(value: Option<&OsStr>) -> Result<ResolvedWasmCFlags, String> {
    let raw = match value {
        Some(value) => Some(
            value
                .to_str()
                .ok_or_else(|| format!("{WASM_CFLAGS_ENV} must be valid UTF-8 compiler flags"))?,
        ),
        None => None,
    };
    resolve_wasm_cflags_from_str(raw)
}

fn resolve_wasm_cflags_from_str(value: Option<&str>) -> Result<ResolvedWasmCFlags, String> {
    let raw = value.unwrap_or("");
    let words = parse_cflags_words(raw)?;
    let mut has_required_target = false;
    for index in 0..words.len() {
        let word = &words[index];
        if word == WASM_C_TARGET_ARG {
            has_required_target = true;
        } else if wasm_cflags_target_conflict(index, &words) {
            return Err(format!(
                "{WASM_CFLAGS_ENV} contains unsupported target flag `{}`; use `{WASM_C_TARGET_ARG}` or omit the target so xtask can prepend it",
                wasm_cflags_target_display(index, &words)
            ));
        }
    }
    let value = if has_required_target {
        raw.to_owned()
    } else {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            WASM_C_TARGET_ARG.to_owned()
        } else {
            format!("{WASM_C_TARGET_ARG} {trimmed}")
        }
    };
    let args = parse_cflags_words(&value)?;
    Ok(ResolvedWasmCFlags { value, args })
}

fn wasm_cflags_target_conflict(index: usize, words: &[String]) -> bool {
    let word = &words[index];
    word == "--target"
        || word == "-target"
        || word.starts_with("--target=")
        || word.starts_with("-target=")
}

fn wasm_cflags_target_display(index: usize, words: &[String]) -> String {
    let word = &words[index];
    if matches!(word.as_str(), "--target" | "-target")
        && let Some(target) = words.get(index + 1)
    {
        return format!("{word} {target}");
    }
    word.clone()
}

fn parse_cflags_words(value: &str) -> Result<Vec<String>, String> {
    let mut words = Vec::new();
    let mut current = String::new();
    let mut quote = CFlagsQuote::None;
    let mut in_word = false;
    let mut chars = value.chars();
    while let Some(character) = chars.next() {
        match quote {
            CFlagsQuote::None => match character {
                character if character.is_whitespace() => {
                    if in_word {
                        words.push(std::mem::take(&mut current));
                        in_word = false;
                    }
                }
                '\'' => {
                    in_word = true;
                    quote = CFlagsQuote::Single;
                }
                '"' => {
                    in_word = true;
                    quote = CFlagsQuote::Double;
                }
                '\\' => {
                    in_word = true;
                    let Some(escaped) = chars.next() else {
                        return Err(format!("{WASM_CFLAGS_ENV} ends with an unfinished escape"));
                    };
                    current.push(escaped);
                }
                character => {
                    in_word = true;
                    current.push(character);
                }
            },
            CFlagsQuote::Single => {
                if character == '\'' {
                    quote = CFlagsQuote::None;
                } else {
                    current.push(character);
                }
            }
            CFlagsQuote::Double => {
                if character == '"' {
                    quote = CFlagsQuote::None;
                } else if character == '\\' {
                    let Some(escaped) = chars.next() else {
                        return Err(format!("{WASM_CFLAGS_ENV} ends with an unfinished escape"));
                    };
                    current.push(escaped);
                } else {
                    current.push(character);
                }
            }
        }
    }
    match quote {
        CFlagsQuote::None => {
            if in_word {
                words.push(current);
            }
            Ok(words)
        }
        CFlagsQuote::Single => Err(format!(
            "{WASM_CFLAGS_ENV} contains an unterminated ' quote"
        )),
        CFlagsQuote::Double => Err(format!(
            "{WASM_CFLAGS_ENV} contains an unterminated \" quote"
        )),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CFlagsQuote {
    None,
    Single,
    Double,
}

fn resolve_explicit_wasm_c_compiler(value: &OsStr, path: &OsStr) -> Result<PathBuf, String> {
    if value.is_empty() {
        return Err(format!(
            "{WASM_C_COMPILER_ENV} is set but empty; set it to a compiler path that accepts `{WASM_C_TARGET_ARG}`"
        ));
    }
    let candidate = PathBuf::from(value);
    if candidate.components().count() > 1 {
        if is_executable_file(&candidate) {
            Ok(candidate)
        } else {
            Err(format!(
                "{WASM_C_COMPILER_ENV} is not executable: {}",
                candidate.display()
            ))
        }
    } else {
        first_executable_match(value, path).ok_or_else(|| {
            format!(
                "{WASM_C_COMPILER_ENV} command was not found on PATH: {}",
                value.to_string_lossy()
            )
        })
    }
}

fn first_executable_match(name: &OsStr, path: &OsStr) -> Option<PathBuf> {
    for dir in env::split_paths(path) {
        let candidate = dir.join(name);
        if is_executable_file(&candidate) {
            return Some(candidate);
        }
    }
    None
}

fn wasm_c_compiler_candidates(path: &OsStr) -> Vec<PathBuf> {
    let mut seen = BTreeSet::new();
    let mut candidates = Vec::new();
    for name in ["wasm32-unknown-unknown-clang", "clang"] {
        for candidate in executable_matches(name, path) {
            let key = fs::canonicalize(&candidate).unwrap_or_else(|_| candidate.clone());
            if seen.insert(key) {
                candidates.push(candidate);
            }
        }
    }
    candidates
}

fn verify_wasm_c_compiler(path: &Path, cflags: &ResolvedWasmCFlags) -> Result<(), String> {
    let tempdir = tempfile::tempdir()
        .map_err(|error| format!("failed to create C probe tempdir: {error}"))?;
    let source_path = tempdir.path().join("wasm_probe.c");
    let object_path = tempdir.path().join("wasm_probe.o");
    fs::write(
        &source_path,
        "int radroots_wasm_probe(void) { return 0; }\n",
    )
    .map_err(|error| format!("failed to write C compiler probe: {error}"))?;
    let output = Command::new(path)
        .args(&cflags.args)
        .arg("-c")
        .arg(&source_path)
        .arg("-o")
        .arg(&object_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|error| format!("failed to run {}: {error}", path.display()))?;
    if output.status.success() && object_path.is_file() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    Err(format!(
        "{} rejected `{WASM_CFLAGS_ENV}={}` for {}: {}",
        path.display(),
        cflags.value,
        WASM_TARGET,
        stderr.trim()
    ))
}

fn prepend_path(command: &mut Command, prefix: &Path) {
    let existing = env::var_os("PATH").unwrap_or_default();
    let mut paths = vec![prefix.to_path_buf()];
    paths.extend(env::split_paths(&existing));
    if let Ok(joined) = env::join_paths(paths) {
        command.env("PATH", joined);
    }
}

#[cfg(test)]
mod tests {
    use std::{
        env,
        ffi::OsStr,
        fs,
        path::{Path, PathBuf},
        process::Command,
        time::{SystemTime, UNIX_EPOCH},
    };

    use crate::package_matrix::wasm_package_specs;

    use super::{
        ResolvedWasmCFlags, WASM_C_COMPILER_ENV, WASM_C_TARGET_ARG, WASM_CFLAGS_ENV, WASM_TARGET,
        WasmCCompiler, remove_wasm_pack_gitignore, resolve_path_tool_from_path,
        resolve_wasm_c_compiler_with, resolve_wasm_cflags_from_str, rust_toolchain_channel_from,
        rustup_target_list_args, selected_specs, target_list_contains, validate_target_list,
        wasm_pack_args, wasm_package_requires_c_compiler,
    };

    #[test]
    fn selects_all_specs_by_default() {
        assert_eq!(selected_specs(&[]).expect("all specs").len(), 3);
    }

    #[test]
    fn selects_one_spec_by_key() {
        let specs = selected_specs(&["--package".to_owned(), "replica_store".to_owned()])
            .expect("replica store spec");
        assert_eq!(specs[0].package_name, "@radroots/replica-store-wasm");
    }

    #[test]
    fn rejects_unknown_spec_key() {
        assert!(selected_specs(&["--package".to_owned(), "missing".to_owned()]).is_err());
    }

    #[test]
    fn wasm_pack_arguments_disable_package_manifest_generation() {
        let args = wasm_pack_args(wasm_package_specs()[0]);
        assert!(args.contains(&"--no-pack"));
    }

    #[test]
    fn removes_wasm_pack_generated_gitignore() {
        let root = test_root("wasm_pack_gitignore");
        let dist_dir = root.join("dist");
        fs::create_dir_all(&dist_dir).expect("create dist");
        fs::write(dist_dir.join(".gitignore"), "*\n").expect("write ignore");

        remove_wasm_pack_gitignore(&dist_dir, wasm_package_specs()[0]).expect("remove ignore");

        assert!(!dist_dir.join(".gitignore").exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn refuses_unexpected_wasm_pack_gitignore_contents() {
        let root = test_root("custom_gitignore");
        let dist_dir = root.join("dist");
        fs::create_dir_all(&dist_dir).expect("create dist");
        fs::write(dist_dir.join(".gitignore"), "!keep\n").expect("write ignore");

        let error = remove_wasm_pack_gitignore(&dist_dir, wasm_package_specs()[0])
            .expect_err("custom ignore rejected");

        assert!(error.contains("unexpected wasm-pack ignore file"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn path_tool_resolution_reports_missing_tools() {
        let error = resolve_path_tool_from_path("wasm-pack", std::ffi::OsStr::new(""))
            .expect_err("missing");
        assert!(error.contains("missing wasm-pack"));
    }

    #[test]
    fn path_tool_resolution_reports_ambiguous_tools() {
        let root = test_root("ambiguous_wasm_pack");
        let first = root.join("first");
        let second = root.join("second");
        fs::create_dir_all(&first).expect("create first dir");
        fs::create_dir_all(&second).expect("create second dir");
        write_executable(first.join("wasm-pack"));
        write_executable(second.join("wasm-pack"));
        let path = env::join_paths([first, second]).expect("join path");

        let error =
            resolve_path_tool_from_path("wasm-pack", &path).expect_err("ambiguous wasm-pack");

        assert!(error.contains("ambiguous wasm-pack"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn target_list_parser_requires_exact_target() {
        assert!(target_list_contains(
            "aarch64-apple-darwin\nwasm32-unknown-unknown\n",
            "wasm32-unknown-unknown"
        ));
        assert!(!target_list_contains(
            "wasm32-unknown-emscripten\n",
            "wasm32-unknown-unknown"
        ));
    }

    #[test]
    fn rustup_target_list_args_are_bound_to_selected_toolchain() {
        assert_eq!(
            rustup_target_list_args("1.92.0"),
            ["target", "list", "--installed", "--toolchain", "1.92.0"]
        );
    }

    #[test]
    fn missing_wasm_target_error_names_selected_toolchain() {
        let error = validate_target_list("aarch64-apple-darwin\n", WASM_TARGET, "1.92.0")
            .expect_err("missing target");

        assert!(error.contains("wasm32-unknown-unknown"));
        assert!(error.contains("--toolchain 1.92.0"));
    }

    #[test]
    fn parses_rust_toolchain_channel() {
        let channel = rust_toolchain_channel_from(
            r#"[toolchain]
channel = "1.92.0"
"#,
        )
        .expect("channel");

        assert_eq!(channel, "1.92.0");
    }

    #[test]
    fn rejects_missing_wasm_c_compiler() {
        let error = resolve_wasm_c_compiler_with(
            None,
            OsStr::new(""),
            Vec::new(),
            wasm_cflags(None),
            |_, _| Ok(()),
        )
        .expect_err("missing compiler");

        assert!(error.contains(WASM_C_COMPILER_ENV));
        assert!(error.contains(WASM_C_TARGET_ARG));
    }

    #[test]
    fn rejects_unsuitable_wasm_c_compiler_candidates() {
        let root = test_root("unsuitable_wasm_c_compiler");
        fs::create_dir_all(&root).expect("create root");
        let compiler = root.join("clang");
        write_executable(compiler.clone());

        let error = resolve_wasm_c_compiler_with(
            None,
            OsStr::new(""),
            vec![compiler],
            wasm_cflags(None),
            |_, _| Err("target unsupported".to_owned()),
        )
        .expect_err("unsuitable compiler");

        assert!(error.contains("missing suitable WASM C compiler"));
        assert!(error.contains(WASM_C_COMPILER_ENV));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn accepts_verified_wasm_c_compiler_candidate() {
        let root = test_root("verified_wasm_c_compiler");
        fs::create_dir_all(&root).expect("create root");
        let compiler = root.join("clang");
        write_executable(compiler.clone());

        let resolved = resolve_wasm_c_compiler_with(
            None,
            OsStr::new(""),
            vec![compiler.clone()],
            wasm_cflags(Some("-O2")),
            |path: &Path, cflags: &ResolvedWasmCFlags| {
                assert_eq!(path, compiler);
                assert_eq!(cflags.value, format!("{WASM_C_TARGET_ARG} -O2"));
                Ok(())
            },
        )
        .expect("compiler");

        assert_eq!(resolved.path, compiler);
        assert_eq!(resolved.cflags.value, format!("{WASM_C_TARGET_ARG} -O2"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn absent_wasm_cflags_resolve_to_required_target() {
        let resolved = resolve_wasm_cflags_from_str(None).expect("cflags");

        assert_eq!(resolved.value, WASM_C_TARGET_ARG);
        assert_eq!(resolved.args, [WASM_C_TARGET_ARG.to_owned()]);
    }

    #[test]
    fn present_valid_wasm_cflags_preserve_additional_flags() {
        let value = "-O2 --target=wasm32-unknown-unknown -fvisibility=hidden";
        let resolved = resolve_wasm_cflags_from_str(Some(value)).expect("cflags");

        assert_eq!(resolved.value, value);
        assert_eq!(
            resolved.args,
            [
                "-O2".to_owned(),
                WASM_C_TARGET_ARG.to_owned(),
                "-fvisibility=hidden".to_owned()
            ]
        );
    }

    #[test]
    fn missing_target_wasm_cflags_prepend_required_target() {
        let resolved = resolve_wasm_cflags_from_str(Some(" -O2 -fPIC ")).expect("target prepended");

        assert_eq!(resolved.value, format!("{WASM_C_TARGET_ARG} -O2 -fPIC"));
        assert_eq!(
            resolved.args,
            [
                WASM_C_TARGET_ARG.to_owned(),
                "-O2".to_owned(),
                "-fPIC".to_owned()
            ]
        );
    }

    #[test]
    fn conflicting_target_wasm_cflags_are_rejected() {
        let error = resolve_wasm_cflags_from_str(Some("--target=wasm32-wasip1 -O2"))
            .expect_err("conflicting target");

        assert!(error.contains(WASM_CFLAGS_ENV));
        assert!(error.contains("--target=wasm32-wasip1"));
        assert!(error.contains(WASM_C_TARGET_ARG));
    }

    #[test]
    fn separated_target_wasm_cflags_are_rejected() {
        let error = resolve_wasm_cflags_from_str(Some("--target wasm32-unknown-unknown"))
            .expect_err("separated target");

        assert!(error.contains("--target wasm32-unknown-unknown"));
        assert!(error.contains(WASM_C_TARGET_ARG));
    }

    #[test]
    fn wasm_c_compiler_command_uses_resolved_cflags() {
        let compiler = WasmCCompiler {
            path: PathBuf::from("clang"),
            cflags: wasm_cflags(Some("-O2")),
        };
        let mut command = Command::new("wasm-pack");

        compiler.apply_to_command(&mut command);

        assert_eq!(
            command_env(&command, WASM_C_COMPILER_ENV),
            Some(std::ffi::OsString::from("clang"))
        );
        assert_eq!(
            command_env(&command, WASM_CFLAGS_ENV),
            Some(std::ffi::OsString::from(format!("{WASM_C_TARGET_ARG} -O2")))
        );
    }

    #[test]
    fn event_codec_wasm_requires_c_compiler() {
        assert!(wasm_package_requires_c_compiler(wasm_package_specs()[0]));
        assert!(!wasm_package_requires_c_compiler(wasm_package_specs()[1]));
    }

    fn wasm_cflags(value: Option<&str>) -> ResolvedWasmCFlags {
        resolve_wasm_cflags_from_str(value).expect("resolved cflags")
    }

    fn command_env(command: &Command, name: &str) -> Option<std::ffi::OsString> {
        command.get_envs().find_map(|(key, value)| {
            if key == OsStr::new(name) {
                value.map(std::ffi::OsStr::to_os_string)
            } else {
                None
            }
        })
    }

    fn write_executable(path: PathBuf) {
        fs::write(&path, "#!/bin/sh\n").expect("write executable");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut permissions = fs::metadata(&path).expect("metadata").permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(&path, permissions).expect("set executable permissions");
        }
    }

    fn test_root(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        env::temp_dir().join(format!("radroots_sdk_xtask_{name}_{stamp}"))
    }
}
