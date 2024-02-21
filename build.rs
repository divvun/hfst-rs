use std::path::Path;

fn main() {
    let dst = cmake::Config::new("lib").always_configure(true).no_build_target(true).build();
    println!("cargo:rustc-link-search=native={}/libhfst", dst.display());

    cc::Build::new()
        .file("wrapper/wrapper.cpp")
        // .include("/usr/local/include/hfst")
        .include("/opt/homebrew/include")
        .include(Path::new("lib/libhfst/src"))
        .static_flag(true)
        .cpp(true)
        .flag("-std=c++11")
        .compile("hfst_wrapper");

}
