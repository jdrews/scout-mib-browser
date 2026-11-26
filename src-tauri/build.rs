// Build script (launch-beta/09): compiles the Tauri capability set into the
// binary and keeps the e2e WebDriver plugin's capability out of production.
//
// Capability collection is gated on the `wdio` cargo feature — the same
// feature that compiles the plugin itself:
//
// - Production (`cargo build` / `tauri build`): only `capabilities/default.json`
//   (core:default + event listening) is collected. `capabilities/wdio.json`
//   is never present in a production build.
// - Test builds (`--features scout-mib-browser/wdio`, used by `tauri dev` and
//   the e2e suite): both files are collected, so the wdio plugin's commands
//   are permitted.
//
// A custom pattern must emit its own rerun-if-changed (tauri-build cannot
// know which files the glob will match later).
fn main() {
    let pattern = if std::env::var("CARGO_FEATURE_WDIO").is_ok() {
        "capabilities/*.json"
    } else {
        "capabilities/default.json"
    };
    println!("cargo::rerun-if-changed=capabilities");
    println!("cargo::rerun-if-changed=tauri.conf.json");
    tauri_build::try_build(tauri_build::Attributes::new().capabilities_path_pattern(pattern))
        .expect("failed to run tauri-build");
}
