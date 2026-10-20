fn main() {
    // Dev builds look for sidecars as ../binaries/<name>-<target-triple>, Tauri's naming scheme.
    println!(
        "cargo:rustc-env=TX_TARGET_TRIPLE={}",
        std::env::var("TARGET").unwrap()
    );
}
