use std::path::{Path, PathBuf};

fn main() {
    let (includes, libs) = if cfg!(windows) {
        let lib = vcpkg::Config::new().find_package("icu").unwrap();
        (lib.include_paths, lib.link_paths)
    } else if cfg!(target_os = "macos") {
        (vec![PathBuf::from("/opt/homebrew/include")], vec![])
    } else {
        (vec![], vec![])
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
        println!("cargo:rustc-link-lib=hfst");
        println!("cargo:rustc-link-lib=icuuc");
        println!("cargo:rustc-link-lib=icuio");
        println!("cargo:rustc-link-lib=icudt");
        println!("cargo:rustc-link-lib=icuin");
    }

    if cfg!(target_os = "macos") {
        println!("cargo:rustc-link-search=native=/opt/homebrew/lib");
    }

    if cfg!(unix) {
        println!(
            "cargo:rustc-link-search=native={}",
            dst.join("build").join("libhfst").display()
        );
        println!("cargo:rustc-link-lib=static=hfst");
        println!("cargo:rustc-link-lib=static=icuuc");
        println!("cargo:rustc-link-lib=static=icuio");
        println!("cargo:rustc-link-lib=static=icudata");
        println!("cargo:rustc-link-lib=static=icui18n");
    }

    if cfg!(target_os = "linux") {
        let o = std::process::Command::new("ld")
            .arg("--verbose")
            .output()
            .unwrap();
        let out = std::str::from_utf8(&o.stdout).unwrap();
        for line in out.lines() {
            if line.starts_with("SEARCH_DIR") {
                let iter = line.split(";").map(|x| {
                    x.trim()
                        .trim_start_matches("SEARCH_DIR(\"=")
                        .trim_end_matches("\")")
                });
                for i in iter {
                    if Path::new(i).exists() {
                        println!("cargo:rustc-link-search=native={}", i);
                    }
                }
                break;
            }
        }
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
