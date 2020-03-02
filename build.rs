use std::env;

fn main() {
    if env::var("HOST") != env::var("TARGET") {   // assume cross-compiling for Kobo
        println!("cargo:rustc-env=PKG_CONFIG_ALLOW_CROSS=1");
        println!("cargo:rustc-link-search=libs");
    } else {                                      // assume compiling for host
        println!("cargo:rustc-link-search=src/wrapper");
        println!("cargo:rustc-link-lib=lcms2");
        println!("cargo:rustc-link-lib=mupdf-third");
    }
    println!("cargo:rustc-link-lib=z");
    println!("cargo:rustc-link-lib=bz2");
    println!("cargo:rustc-link-lib=jpeg");
    println!("cargo:rustc-link-lib=png16");
    println!("cargo:rustc-link-lib=openjp2");
    println!("cargo:rustc-link-lib=jbig2dec");
    println!("cargo:rustc-link-lib=dylib=stdc++");
}
