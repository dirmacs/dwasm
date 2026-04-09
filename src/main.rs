//! dwasm — Build tool for Leptos WASM frontends.
//!
//! Replaces `trunk build --release` with a pipeline that handles
//! the wasm-opt bulk-memory compatibility issue and automates
//! content hashing + index.html patching.
//!
//! ## Pipeline
//!
//! 1. `cargo build --release --target wasm32-unknown-unknown`
//! 2. `wasm-bindgen --target web --no-typescript`
//! 3. `wasm-opt -Oz --enable-bulk-memory` (skippable)
//! 4. Content-hash filenames and copy to `dist/`
//! 5. Patch `index.html` references and strip integrity hashes
//!
//! ## Usage
//!
//! ```bash
//! # Workspace member crate
//! dwasm --crate-name my-web --project ~/projects/my-app
//!
//! # Standalone crate
//! dwasm --crate-name my-admin --project ~/projects/my-admin --standalone
//!
//! # Skip wasm-opt
//! dwasm --crate-name my-app --project . --standalone --skip-opt
//! ```

use clap::Parser;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Parser)]
#[command(name = "dwasm", about = "Build Leptos WASM frontends with wasm-opt bulk-memory support")]
struct Cli {
    /// Crate name to build (e.g., eruka-web, dirmacs-admin)
    #[arg(long)]
    crate_name: String,

    /// Project root directory (default: current dir)
    #[arg(long, default_value = ".")]
    project: String,

    /// Standalone crate (not a workspace member)
    #[arg(long)]
    standalone: bool,

    /// Skip wasm-opt optimization
    #[arg(long)]
    skip_opt: bool,

    /// wasm-opt binary path (auto-detected from trunk cache or PATH)
    #[arg(long)]
    wasm_opt: Option<String>,

    /// dist directory (default: <crate-dir>/dist)
    #[arg(long)]
    dist: Option<String>,
}

fn main() {
    let cli = Cli::parse();
    let project = PathBuf::from(&cli.project).canonicalize().unwrap_or_else(|_| {
        eprintln!("Project directory '{}' not found", cli.project);
        std::process::exit(1);
    });

    let crate_dir = find_crate_dir(&project, &cli.crate_name, cli.standalone);
    let dist_dir = cli.dist.map(PathBuf::from).unwrap_or_else(|| crate_dir.join("dist"));
    let wasm_name = cli.crate_name.replace('-', "_");

    println!("=== dwasm ===");
    println!("Crate:   {}", cli.crate_name);
    println!("Project: {}", project.display());
    println!("Dist:    {}", dist_dir.display());
    println!();

    // Step 1: cargo build
    step("1/5", "cargo build --release --target wasm32-unknown-unknown");
    let mut build_args = vec![
        "build", "--release", "--target", "wasm32-unknown-unknown",
    ];
    if !cli.standalone {
        build_args.extend_from_slice(&["-p", &cli.crate_name]);
    }
    run_or_exit("cargo", &build_args, &project);

    // Step 2: wasm-bindgen
    step("2/5", "wasm-bindgen");
    let wasm_file = project
        .join("target/wasm32-unknown-unknown/release")
        .join(format!("{}.wasm", wasm_name));
    if !wasm_file.exists() {
        eprintln!("WASM file not found: {}", wasm_file.display());
        std::process::exit(1);
    }

    let bindgen_dir = dist_dir.join(".bindgen");
    std::fs::create_dir_all(&bindgen_dir).ok();
    run_or_exit("wasm-bindgen", &[
        "--out-dir", &bindgen_dir.to_string_lossy(),
        "--target", "web",
        "--no-typescript",
        &wasm_file.to_string_lossy(),
    ], &project);

    let bg_wasm = bindgen_dir.join(format!("{}_bg.wasm", wasm_name));
    let bg_js = bindgen_dir.join(format!("{}.js", wasm_name));

    // Step 3: wasm-opt
    if !cli.skip_opt {
        step("3/5", "wasm-opt -Oz --enable-bulk-memory");
        let wasm_opt_bin = cli.wasm_opt.unwrap_or_else(find_wasm_opt);
        let opt_output = bindgen_dir.join("optimized.wasm");

        let result = Command::new(&wasm_opt_bin)
            .args(["--enable-bulk-memory", "-Oz",
                   &bg_wasm.to_string_lossy(), "-o", &opt_output.to_string_lossy()])
            .status();

        match result {
            Ok(s) if s.success() => {
                let before = std::fs::metadata(&bg_wasm).map(|m| m.len()).unwrap_or(0);
                std::fs::rename(&opt_output, &bg_wasm).ok();
                let after = std::fs::metadata(&bg_wasm).map(|m| m.len()).unwrap_or(0);
                println!("  {:.1} MB → {:.1} MB ({:.0}% reduction)",
                    before as f64 / 1_048_576.0,
                    after as f64 / 1_048_576.0,
                    (1.0 - after as f64 / before as f64) * 100.0);
            }
            _ => eprintln!("  wasm-opt failed, continuing without optimization"),
        }
    } else {
        step("3/5", "skipped (--skip-opt)");
    }

    // Step 4: Hash and copy
    step("4/5", "content-hash and copy to dist");
    std::fs::create_dir_all(&dist_dir).ok();
    clean_old_artifacts(&dist_dir, &cli.crate_name, &wasm_name);

    let wasm_bytes = std::fs::read(&bg_wasm).expect("Failed to read WASM");
    let hash = content_hash(&wasm_bytes);

    let final_wasm = format!("{}-{}_bg.wasm", cli.crate_name, hash);
    let final_js = format!("{}-{}.js", cli.crate_name, hash);

    std::fs::copy(&bg_wasm, dist_dir.join(&final_wasm)).expect("copy WASM");

    let js_content = std::fs::read_to_string(&bg_js).expect("read JS");
    let patched_js = js_content.replace(
        &format!("{}_bg.wasm", wasm_name),
        &format!("/{}", final_wasm),
    );
    std::fs::write(dist_dir.join(&final_js), patched_js).expect("write JS");

    // Step 5: Patch index.html
    step("5/5", "patch index.html");
    patch_index_html(&dist_dir, &cli.crate_name, &final_js, &final_wasm);

    // Cleanup
    std::fs::remove_dir_all(&bindgen_dir).ok();

    let wasm_size = wasm_bytes.len();
    println!();
    println!("=== Done ===");
    println!("WASM: {} ({:.1} MB)", final_wasm, wasm_size as f64 / 1_048_576.0);
    println!("JS:   {}", final_js);
}

fn find_crate_dir(project: &Path, crate_name: &str, standalone: bool) -> PathBuf {
    if standalone {
        return project.to_path_buf();
    }
    let candidates = [
        project.join("crates").join(crate_name),
        project.join(crate_name),
    ];
    for c in &candidates {
        if c.join("Cargo.toml").exists() {
            return c.clone();
        }
    }
    eprintln!("Crate '{}' not found in '{}'", crate_name, project.display());
    std::process::exit(1);
}

fn clean_old_artifacts(dist: &Path, crate_name: &str, wasm_name: &str) {
    let prefixes = [crate_name.to_string(), wasm_name.to_string()];
    if let Ok(entries) = std::fs::read_dir(dist) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            let is_artifact = prefixes.iter().any(|p| name.starts_with(p))
                && (name.ends_with(".wasm") || name.ends_with(".js"));
            if is_artifact {
                std::fs::remove_file(entry.path()).ok();
            }
        }
    }
}

fn patch_index_html(dist: &Path, crate_name: &str, new_js: &str, new_wasm: &str) {
    let index = dist.join("index.html");
    if !index.exists() {
        eprintln!("  No index.html found");
        return;
    }

    let html = std::fs::read_to_string(&index).expect("read index.html");
    let mut updated = html.clone();

    // Replace old hashed references: crate-name-OLDHASH.js / crate-name-OLDHASH_bg.wasm
    let prefix = format!("{}-", crate_name);
    for line in html.lines() {
        if !line.contains(&prefix) { continue; }
        // Find JS references
        if let Some(old) = extract_reference(line, &prefix, ".js") {
            updated = updated.replace(&old, new_js);
        }
        // Find WASM references
        if let Some(old) = extract_reference(line, &prefix, "_bg.wasm") {
            updated = updated.replace(&old, new_wasm);
        }
    }

    // Strip integrity attributes
    while let Some(start) = updated.find(" integrity=\"") {
        if let Some(end) = updated[start + 12..].find('"') {
            updated = format!("{}{}", &updated[..start], &updated[start + 12 + end + 1..]);
        } else {
            break;
        }
    }

    std::fs::write(&index, updated).expect("write index.html");
}

fn extract_reference(line: &str, prefix: &str, suffix: &str) -> Option<String> {
    let start = line.find(prefix)?;
    let end = line[start..].find(suffix)?;
    Some(line[start..start + end + suffix.len()].to_string())
}

fn content_hash(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hex::encode(&hasher.finalize()[..8])
}

fn find_wasm_opt() -> String {
    // Check trunk cache
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    let cache = PathBuf::from(&home).join(".cache/trunk");
    if let Ok(entries) = std::fs::read_dir(&cache) {
        for entry in entries.flatten() {
            let bin = entry.path().join("bin/wasm-opt");
            if bin.exists() {
                return bin.to_string_lossy().to_string();
            }
        }
    }
    "wasm-opt".to_string()
}

fn step(num: &str, msg: &str) {
    println!("[{}] {}", num, msg);
}

fn run_or_exit(cmd: &str, args: &[&str], dir: &Path) {
    let status = Command::new(cmd)
        .args(args)
        .current_dir(dir)
        .status()
        .unwrap_or_else(|e| {
            eprintln!("Failed to run {}: {}", cmd, e);
            std::process::exit(1);
        });
    if !status.success() {
        eprintln!("{} failed with exit code {:?}", cmd, status.code());
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_content_hash_deterministic() {
        let h1 = content_hash(b"hello world");
        let h2 = content_hash(b"hello world");
        assert_eq!(h1, h2);
        assert_eq!(h1.len(), 16); // 8 bytes = 16 hex chars
    }

    #[test]
    fn test_content_hash_different_inputs() {
        let h1 = content_hash(b"hello");
        let h2 = content_hash(b"world");
        assert_ne!(h1, h2);
    }

    #[test]
    fn test_extract_reference_js() {
        let line = r#"import init from '/eruka-web-abc123def456.js';"#;
        let result = extract_reference(line, "eruka-web-", ".js");
        assert_eq!(result, Some("eruka-web-abc123def456.js".to_string()));
    }

    #[test]
    fn test_extract_reference_wasm() {
        let line = r#"const wasm = '/eruka-web-abc123def456_bg.wasm';"#;
        let result = extract_reference(line, "eruka-web-", "_bg.wasm");
        assert_eq!(result, Some("eruka-web-abc123def456_bg.wasm".to_string()));
    }

    #[test]
    fn test_extract_reference_no_match() {
        let line = "no references here";
        assert_eq!(extract_reference(line, "eruka-web-", ".js"), None);
    }

    #[test]
    fn test_clean_old_artifacts_patterns() {
        // Verify the prefix matching logic
        let prefixes = ["my-app", "my_app"];
        let should_clean = ["my-app-abc123.js", "my-app-abc123_bg.wasm", "my_app-xyz.js"];
        let should_keep = ["other-app-abc.js", "style.css", "index.html"];

        for name in should_clean {
            let matches = prefixes.iter().any(|p| name.starts_with(p))
                && (name.ends_with(".wasm") || name.ends_with(".js"));
            assert!(matches, "'{}' should be cleaned", name);
        }
        for name in should_keep {
            let matches = prefixes.iter().any(|p| name.starts_with(p))
                && (name.ends_with(".wasm") || name.ends_with(".js"));
            assert!(!matches, "'{}' should be kept", name);
        }
    }
}
