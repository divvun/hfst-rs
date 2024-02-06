fn main() {
    println!("cargo:rustc-link-lib=hfst");

    cc::Build::new()
        .file("wrapper/wrapper.cpp")
        .include("/usr/local/include/hfst")
        .include("/opt/homebrew/include")
        .static_flag(true)
        .cpp(true)
        .flag("-std=c++11")
        .compile("hfst_wrapper");
}
