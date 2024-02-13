#[cfg(unix)]
fn main() {
    println!("cargo:rustc-link-lib=hfst");

    // let include = std::env::var("HFST_INCLUDE").ok();

    cc::Build::new()
        .file("wrapper/wrapper.cpp")
        .include("/usr/local/include/hfst")
        .include("/opt/homebrew/include")
        .static_flag(true)
        .cpp(true)
        .flag("-std=c++11")
        .compile("hfst_wrapper");
}

#[cfg(windows)]
fn main() {
    use std::path::PathBuf;

    let sysroot = PathBuf::from(std::env::var("SYSROOT").unwrap());

    println!("cargo:rustc-link-search=native={}/lib", sysroot.display());
    println!("cargo:rustc-link-search=native={}/bin", sysroot.display());
    println!("cargo:rustc-link-lib=dylib=hfst");

    cc::Build::new()
        .file("wrapper/wrapper.cpp")
        .include(sysroot.join("include").join("hfst"))
        .include(sysroot.join("include"))
        .cpp(true)
        .compile("hfst_wrapper");
}
