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

    #[cfg(windows)]
    windows_resources(&version);
}

/// Icon and version info of armory.exe, shown by Explorer and the taskbar.
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
            "A.R.M.O.R.Y. - Arcade Runner Menu On Rails, Yeah!",
        )
        .set("ProductVersion", version)
        .set("LegalCopyright", "MIT License");
    if let Err(e) = res.compile() {
        panic!("Failed to embed the Windows resources: {e}");
    }
}
