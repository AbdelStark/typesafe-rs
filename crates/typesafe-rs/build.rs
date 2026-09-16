//! Captures rustc version and target triple for `X-TypeSafe-Runtime`.

fn main() {
    println!("cargo:rerun-if-env-changed=RUSTC");

    let rustc = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".to_owned());
    let version = std::process::Command::new(&rustc)
        .arg("--version")
        .output()
        .ok()
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .and_then(|stdout| stdout.split_whitespace().nth(1).map(str::to_owned))
        .unwrap_or_else(|| "unknown".to_owned());
    println!("cargo:rustc-env=TYPESAFE_RUSTC_VERSION={version}");

    let os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_else(|_| "unknown".to_owned());
    let arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_else(|_| "unknown".to_owned());
    println!("cargo:rustc-env=TYPESAFE_TARGET_OS={os}");
    println!("cargo:rustc-env=TYPESAFE_TARGET_ARCH={arch}");
}
