use cc::Build;

fn main() {
    let mut build = Build::new();
    build
        .cpp(true)
        .flag_if_supported("/std:c++11")
        .flag_if_supported("-w");

    println!("cargo:rerun-if-changed=src/alloc_internal.cpp");
    build.file("src/alloc_internal.cpp");
    build.compile("alloc_internal");
}
