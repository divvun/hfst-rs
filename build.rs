fn main() {
    // println!("cargo:rustc-link-lib=hfst");

    let dst = cmake::Config::new("lib").always_configure(true).no_build_target(true).build();
    println!("cargo:rustc-link-search=native={}/libhfst", dst.display());

    cc::Build::new()
        .file("wrapper/wrapper.cpp")
        // .include("/usr/local/include/hfst")
        // .include("/opt/homebrew/include")
        .include(dst.join("include"))
        .static_flag(true)
        .cpp(true)
        .flag("-std=c++11")
        .compile("hfst_wrapper");
// }
// fn main() {
    // let icu = pkg_config::Config::new()
    //     .statik(true)
    //     .probe("icu-uc")
    //     .unwrap();
    // for path in icu.link_paths {
    //     println!("cargo:rustc-link-search=native={}", path.display());
    // }

    // println!("cargo:rustc-link-lib=static=cg3");
    // println!("cargo:rustc-link-lib=static=icuuc");
    // println!("cargo:rustc-link-lib=static=icuio");
    // println!("cargo:rustc-link-lib=static=icudata");
    // println!("cargo:rustc-link-lib=static=icui18n");

    // cc::Build::new()
    //     .file("wrapper/wrapper.cpp")
    //     .include(dst.join("include"))
    //     .include(dst.join("include").join("cg3"))
    //     .include(dst)
    //     .static_flag(true)
    //     .cpp(true)
    //     .flag("-std=c++11")
    //     .compile("cg3_wrapper");
}
