//! companion plugin wasmのbuild・埋込（DD-10.3。設計レビューR4）。
//!
//! - cargo実行ファイルは`CARGO` envで解決（未設定なら`cargo`）
//! - `<cargo> build -p zelper-companion-plugin --release --locked
//!    --target wasm32-wasip1 --target-dir <workspace>/target/plugin-build`。
//!   `-p`明示でworkspace全体の再帰buildを防止し、target-dir分離で親cargoの
//!   target dir lockと衝突しない
//! - artifactを`OUT_DIR/zelper-companion.wasm`へcopyし、zelper crateは
//!   `include_bytes!`で読む（src/companion.rs）。escape hatch（ZELPER_PLUGIN_WASM）
//!   指定時も同一の参照pathへcopyする
//! - escape hatch: `ZELPER_PLUGIN_WASM` envにprebuilt wasm pathを指定すると
//!   plugin buildを省略しそのfileを埋め込む（CI高速化等）

use std::path::PathBuf;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=plugin/");
    println!("cargo:rerun-if-changed=plugin/src/");
    println!("cargo:rerun-if-env-changed=ZELPER_PLUGIN_WASM");

    let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR is set by cargo"));
    let dest = out_dir.join("zelper-companion.wasm");

    if let Ok(prebuilt) = std::env::var("ZELPER_PLUGIN_WASM") {
        std::fs::copy(&prebuilt, &dest).unwrap_or_else(|e| {
            panic!(
                "ZELPER_PLUGIN_WASM copy failed ({prebuilt} -> {}): {e}",
                dest.display()
            )
        });
        return;
    }

    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let manifest_dir =
        PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is set"));
    // workspace root = zelper crateのmanifest dir（plugin/はその直下のmember）
    let target_dir = manifest_dir.join("target").join("plugin-build");

    let output = Command::new(&cargo)
        .arg("build")
        .arg("-p")
        .arg("zelper-companion-plugin")
        .arg("--release")
        .arg("--locked")
        .arg("--target")
        .arg("wasm32-wasip1")
        .arg("--target-dir")
        .arg(&target_dir)
        .output()
        .unwrap_or_else(|e| panic!("failed to run cargo ({cargo}): {e}"));

    if !output.status.success() {
        panic!(
            "companion plugin build failed ({}). ensure the wasm32-wasip1 target is \
installed (rustup target add wasm32-wasip1). stderr:\n{}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let artifact = target_dir
        .join("wasm32-wasip1")
        .join("release")
        .join("zelper-companion-plugin.wasm");
    std::fs::copy(&artifact, &dest)
        .unwrap_or_else(|e| panic!("failed to copy {}: {e}", artifact.display()));
}
