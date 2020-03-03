use std::env;

fn main() {
    let quadruple = env::var("TARGET").unwrap();
    if quadruple == "arm-unknown-linux-gnueabihf" { //=> assume cross-compiling for Kobo
        println!("cargo:rustc-env=PKG_CONFIG_ALLOW_CROSS=1");

        println!("cargo:rustc-link-search=src/mupdf_wrapper/Kobo");
        println!("cargo:rustc-link-search=libs");

        println!("cargo:rustc-link-lib=dylib=stdc++");

    } else {
        let target = env::var("CARGO_CFG_TARGET_OS").unwrap();
        if target == "linux" {                      //=> assume compiling for Linux host
            println!("cargo:rustc-link-search=src/mupdf_wrapper/Linux");

            println!("cargo:rustc-link-lib=mupdf-third");
            println!("cargo:rustc-link-lib=dylib=stdc++");

        } else if target == "macos" {               //=> assume compiling for MacOS host
            println!("cargo:rustc-link-search=src/mupdf_wrapper/Darwin");

            println!("cargo:rustc-link-lib=mupdf-third");
            println!("cargo:rustc-link-lib=dylib=c++");

        } else {
            panic!("Only linux and macos hosts are supported for now");
        }
    }    //=> In any case...
    println!("cargo:rustc-link-lib=z");
    println!("cargo:rustc-link-lib=bz2");
    println!("cargo:rustc-link-lib=jpeg");
    println!("cargo:rustc-link-lib=png16");
    println!("cargo:rustc-link-lib=openjp2");
    println!("cargo:rustc-link-lib=jbig2dec");
}
