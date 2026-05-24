use std::error::Error;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use cargo_metadata::Message;

use crate::Arguments;

/// Build flags passed through to `cargo build` as-is (no following parameter).
const PASSTHROUGH_FLAGS: &[&str] = &["--release", "--all-features", "--no-default-features"];

/// Build flags passed through to `cargo build` together with their following parameter value.
const PASSTHROUGH_FLAGS_WITH_PARAMETER: &[&str] = &[
    "--bin",
    "--example",
    "--features",
    "-F",
    "--profile",
    "--target",
    "--package",
    "-p",
    "--manifest-path",
];

/// Run `cargo build --message-format=json` and return the path to the produced
/// executable artifact. Stderr (build progress) is forwarded to the terminal.
pub fn build_and_find_elf(arguments: &Arguments) -> Result<PathBuf, Box<dyn Error>> {
    let mut cmd = Command::new("cargo");
    cmd.args(["build", "--message-format=json"]);

    for &flag in PASSTHROUGH_FLAGS {
        if flag_is_set(arguments, flag) {
            cmd.arg(flag);
        }
    }
    for &flag in PASSTHROUGH_FLAGS_WITH_PARAMETER {
        if let Some(val) = flag_value(arguments, flag) {
            cmd.args([flag, val.as_str()]);
        }
    }

    cmd.stdout(Stdio::piped());
    // Forward build progress / errors to the terminal.
    cmd.stderr(Stdio::inherit());

    let mut child = cmd.spawn()?;
    let stdout = child.stdout.take().expect("stdout was piped");

    let mut elf_path: Option<PathBuf> = None;
    let mut artifact_count = 0usize;

    for message in Message::parse_stream(std::io::BufReader::new(stdout)) {
        if let Message::CompilerArtifact(artifact) = message?
            && let Some(executable) = artifact.executable
        {
            artifact_count += 1;
            if elf_path.is_none() {
                elf_path = Some(executable.into());
            }
        }
    }

    let status = child.wait()?;
    if !status.success() {
        return Err("cargo build failed".into());
    }

    if artifact_count > 1 && arguments.bin.is_none() && arguments.example.is_none() {
        eprintln!(
            "Warning: multiple executable artifacts found; using the first one. \
             Use --bin <NAME> to select a specific target."
        );
    }

    elf_path.ok_or_else(|| "cargo build produced no executable artifact".into())
}

pub fn find_memory_x(elf_path: &std::path::Path) -> Option<PathBuf> {
    // Derive the profile directory and binary name from the ELF path.
    // Expected layout: <target_dir>/<triple>/<profile>/<name>
    let binary_name = elf_path.file_name()?.to_str()?;
    let binary_directory = elf_path.parent()?;
    let fingerprint_directory = binary_directory.join(".fingerprint");
    let build_directory = binary_directory.join("build");

    // Step 1: find the fingerprint dir containing dep-bin-<name>.
    // The dir is named <package_name>-{HASH}, which may differ from the binary name.
    let dep_bin_name = format!("dep-bin-{}", binary_name);
    let fingerprint_subdirectory = std::fs::read_dir(&fingerprint_directory)
        .ok()?
        .filter_map(|e| e.ok())
        .find_map(|entry| {
            let dep_bin = entry.path().join(&dep_bin_name);
            if dep_bin.exists() {
                entry.file_name().to_str().map(str::to_string)
            } else {
                None
            }
        })?;

    // Step 2: parse the fingerprint JSON to extract the build_script_build hash X.
    // The JSON file is named bin-<name>.json inside the fingerprint dir.
    let json_path = fingerprint_directory
        .join(&fingerprint_subdirectory)
        .join(format!("bin-{}.json", binary_name));
    let json_str = std::fs::read_to_string(&json_path).ok()?;
    let json: serde_json::Value = serde_json::from_str(&json_str).ok()?;

    let build_script_hash_value: u64 = json["deps"]
        .as_array()?
        .iter()
        .find(|dep| dep.get(1).and_then(|v| v.as_str()) == Some("build_script_build"))?
        .get(3)?
        .as_u64()?;

    // Step 3: find the run-build-script file whose contents match hash X, and
    // return the full fingerprint directory name (<crate-name>-{BS_HASH}).
    let target_hex = format!("{:x}", build_script_hash_value.to_be());

    let build_script_directory = std::fs::read_dir(&fingerprint_directory)
        .ok()?
        .filter_map(|e| e.ok())
        .find_map(|entry| {
            let run_file = entry.path().join("run-build-script-build-script-build");
            if !run_file.exists() {
                return None;
            }
            let contents = std::fs::read_to_string(&run_file).ok()?;
            if contents.trim() == target_hex {
                entry.file_name().to_str().map(str::to_string)
            } else {
                None
            }
        })?;

    // Step 4: read root-output to get OUT_DIR.
    let root_output_path = build_directory
        .join(&build_script_directory)
        .join("root-output");
    let out_dir = std::fs::read_to_string(&root_output_path)
        .ok()
        .map(|s| PathBuf::from(s.trim()))?;

    // Step 5: return OUT_DIR/memory.x if it exists.
    let memory_x = out_dir.join("memory.x");
    if memory_x.exists() {
        Some(memory_x)
    } else {
        None
    }
}

fn flag_is_set(arguments: &Arguments, flag: &str) -> bool {
    match flag {
        "--release" => arguments.release,
        "--all-features" => arguments.all_features,
        "--no-default-features" => arguments.no_default_features,
        _ => false,
    }
}

/// Returns the value for the given parameter flag from `arguments`, or `None`.
fn flag_value(arguments: &Arguments, flag: &str) -> Option<String> {
    match flag {
        "--bin" => arguments.bin.clone(),
        "--example" => arguments.example.clone(),
        "--features" | "-F" => arguments.features.clone(),
        "--profile" => arguments.profile.clone(),
        "--target" => arguments.target.clone(),
        "--package" | "-p" => arguments.package.clone(),
        "--manifest-path" => arguments
            .manifest_path
            .as_ref()
            .map(|p| p.to_string_lossy().into_owned()),
        _ => None,
    }
}
