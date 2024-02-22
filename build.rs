use std::path::{Path, PathBuf};

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
    println!(
        "cargo:rustc-link-search=native={}/build/libhfst",
        dst.display()
    );
    println!("cargo:rustc-link-lib=static=hfst");

    cc::Build::new()
        .file("wrapper/wrapper.cpp")
        .includes(includes)
        .include(Path::new("lib").join("libhfst").join("src"))
        .static_flag(true)
        .cpp(true)
        .flag(if cfg!(windows) {
            "/std:c++14"
        } else {
            "-std=c++11"
        })
        .compile("hfst_wrapper");
}
