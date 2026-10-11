fn main() {
    // CI injects the version computed by scripts/version.sh, as for the launcher (see the root
    // build.rs). Cargo.toml stays at 0.0.0, so local builds are marked as unofficial.
    println!("cargo:rerun-if-env-changed=ARMORY_VERSION");
    let version = std::env::var("ARMORY_VERSION")
        .ok()
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| env!("CARGO_PKG_VERSION").to_string());
    println!("cargo:rustc-env=ARMORY_VERSION={version}");

    #[cfg(windows)]
    windows_resources(&version);
}

/// Icon and version info of launchbox-images.exe, shown by Explorer.
#[cfg(windows)]
fn windows_resources(version: &str) {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    println!("cargo:rerun-if-changed=images/icon.ico");
    let mut res = winresource::WindowsResource::new();
    res.set_icon("images/icon.ico")
        .set("ProductName", "A.R.M.O.R.Y.")
        .set(
            "FileDescription",
            "A.R.M.O.R.Y. LaunchBox images - clear logos as button images",
        )
        .set("ProductVersion", version)
        .set("LegalCopyright", "MIT License");
    if let Err(e) = res.compile() {
        panic!("Failed to embed the Windows resources: {e}");
    }
}
