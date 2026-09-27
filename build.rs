fn main() {
    // Compile the Slint UI; the generated code is included via slint::include_modules!().
    if let Err(e) = slint_build::compile("ui/app.slint") {
        panic!("Failed to compile ui/app.slint: {e}");
    }

    // CI injects the version computed by scripts/version.sh. Cargo.toml stays at 0.0.0, so
    // local builds are marked as unofficial.
    println!("cargo:rerun-if-env-changed=ARMORY_VERSION");
    let version = std::env::var("ARMORY_VERSION")
        .ok()
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| env!("CARGO_PKG_VERSION").to_string());
    println!("cargo:rustc-env=ARMORY_VERSION={version}");
}
