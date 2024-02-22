use std::path::PathBuf;

fn main() {
    let includes = if cfg!(windows) {
        let lib = vcpkg::Config::new().find_package("icu").unwrap();
        lib.include_paths
    } else if cfg!(target_os = "macos") {
        vec![PathBuf::from("/opt/homebrew/include")]
    } else {
        vec![]
    };

    let dst = cmake::Config::new("lib")
        .always_configure(true)
        .define(
            "CMAKE_CXX_FLAGS",
            if cfg!(windows) { "/EHsc /O2" } else { "-O2" },
        )
        .no_build_target(true)
        .build();

    if cfg!(windows) {
        println!(
            "cargo:rustc-link-search=native={}",
            dst.join("build")
                .join("libhfst")
                .join(std::env::var("PROFILE").unwrap())
                .display()
        );
    } else {
        println!(
            "cargo:rustc-link-search=native={}",
            dst.join("build").join("libhfst").display()
        );
    }
    
    println!("cargo:rustc-link-lib=hfst");
    println!("cargo:rustc-link-lib=icuuc");
    println!("cargo:rustc-link-lib=icuio");
    if cfg!(windows) {
        println!("cargo:rustc-link-lib=icudt");
        println!("cargo:rustc-link-lib=icuin");
    } else {
        println!("cargo:rustc-link-lib=icudata");
        println!("cargo:rustc-link-lib=icui18n");
    }

    let is_shared = cfg!(windows) && std::env::var("VCPKGRS_DYNAMIC").is_ok();

    cc::Build::new()
        .file("wrapper/wrapper.cpp")
        .includes(includes)
        .include(
            std::env::current_dir()
                .unwrap()
                .join("lib")
                .join("libhfst")
                .join("src"),
        )
        .static_flag(!is_shared)
        .static_crt(!is_shared)
        .cpp(true)
        .flag(if cfg!(windows) {
            "/std:c++14"
        } else {
            "-std=c++11"
        })
        .compile("hfst_wrapper");
}
