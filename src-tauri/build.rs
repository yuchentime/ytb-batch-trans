fn main() {
  tauri_build::build();
  declare_windows_manifest_dependencies();
}

#[cfg(target_os = "windows")]
fn declare_windows_manifest_dependencies() {
  // The dependency tree statically imports `TaskDialogIndirect` from comctl32. Without an
  // image manifest requesting the v6 side-by-side assembly, Windows loads comctl32 v5 and
  // `cargo test` / `cargo build` binaries die at load with STATUS_ENTRYPOINT_NOT_FOUND
  // (0xc0000139). The Rust CI gates run on Linux only, so this never surfaced there.
  const COMMON_CONTROLS_V6: &str = "/MANIFESTDEPENDENCY:type='win32' \
    name='Microsoft.Windows.Common-Controls' version='6.0.0.0' \
    processorArchitecture='*' publicKeyToken='6595b64144ccf1df' language='*'";

  // `rustc-link-arg` covers the unit-test binary built from the lib (there is no separate
  // integration-test target), plus bins/cdylib.
  println!("cargo:rustc-link-arg={COMMON_CONTROLS_V6}");
}

#[cfg(not(target_os = "windows"))]
fn declare_windows_manifest_dependencies() {}
