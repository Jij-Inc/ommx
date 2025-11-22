use std::env;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=wrapper.h");

    // Link to SCIP library
    // Users should have SCIP installed on their system
    // On Ubuntu/Debian: apt-get install libscip-dev
    // On macOS: brew install scip
    println!("cargo:rustc-link-lib=scip");

    // Try to find SCIP using pkg-config
    // If pkg-config is not available or SCIP is not found,
    // we'll continue and let the user set the library path manually
    if let Ok(scip_lib) = pkg_config::Config::new().probe("scip") {
        for path in scip_lib.link_paths {
            println!("cargo:rustc-link-search=native={}", path.display());
        }
    } else {
        // Fallback to common installation paths
        println!("cargo:warning=pkg-config for SCIP not found, trying common paths");
        println!("cargo:rustc-link-search=/usr/local/lib");
        println!("cargo:rustc-link-search=/usr/lib");
        println!("cargo:rustc-link-search=/opt/scip/lib");
    }

    // Generate bindings using bindgen
    let bindings = bindgen::Builder::default()
        .header("wrapper.h")
        // Tell bindgen to use clang arguments from pkg-config if available
        .clang_args(
            pkg_config::Config::new()
                .probe("scip")
                .ok()
                .map(|scip| {
                    scip.include_paths
                        .iter()
                        .map(|p| format!("-I{}", p.display()))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_else(|| {
                    vec![
                        "-I/usr/local/include".to_string(),
                        "-I/usr/include".to_string(),
                        "-I/opt/scip/include".to_string(),
                    ]
                }),
        )
        // Allowlist SCIP functions and types
        .allowlist_function("SCIP.*")
        .allowlist_type("SCIP.*")
        .allowlist_var("SCIP.*")
        // Derive common traits
        .derive_debug(true)
        .derive_default(true)
        // Generate Rust bindings
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .generate()
        .expect("Unable to generate SCIP bindings");

    // Write bindings to output directory
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("scip_bindings.rs"))
        .expect("Couldn't write bindings!");
}
