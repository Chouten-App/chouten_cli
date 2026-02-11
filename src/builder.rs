use anyhow::Result;
use std::path::{Path, PathBuf};
use tokio::process::Command;
use tokio::fs;
use toml::Value;

/// Builds the module at `module_path` and returns the final .wasm path
pub async fn build_module(module_path: &Path) -> Result<PathBuf> {
    // 1️⃣ Read Cargo.toml
    let cargo_toml_path = module_path.join("Cargo.toml");
    let toml_str = fs::read_to_string(&cargo_toml_path).await?;
    let toml_value: Value = toml::from_str(&toml_str)?;

    // 2️⃣ Determine crate name
    let crate_name = toml_value
        .get("lib")
        .and_then(|lib| lib.get("name"))
        .or_else(|| toml_value.get("package").and_then(|pkg| pkg.get("name")))
        .and_then(|n| n.as_str())
        .ok_or_else(|| anyhow::anyhow!("Cannot determine crate name"))?;

    // 3️⃣ Check optional [chouten] section for custom wasm filename
    let wasm_file = toml_value
        .get("chouten")
        .and_then(|c| c.get("wasm_file"))
        .and_then(|s| s.as_str())
        .unwrap_or(crate_name);

    // 4️⃣ Run cargo build for wasm target
    let status = Command::new("cargo")
        .current_dir(module_path)
        .args(&["build", "--release", "--target", "wasm32-unknown-unknown"])
        .status()
        .await?;

    if !status.success() {
        anyhow::bail!("Cargo build failed");
    }

    // 5️⃣ Resolve wasm output path
    let wasm_path = module_path
        .join("target/wasm32-unknown-unknown/release")
        .join(format!("{wasm_file}"));

    if !wasm_path.exists() {
        anyhow::bail!(
            "WASM file not found: {}. Did you set crate-type = [\"cdylib\"] in Cargo.toml?",
            wasm_path.display()
        );
    }

    Ok(wasm_path)
}

