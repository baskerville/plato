use std::env;

fn main() {
    if env::var("HOST") != env::var("TARGET") {
        println!("cargo:rustc-flags=-L libs -l jpeg -l png16 -l openjp2 -l jbig2dec -l bz2 -l z -l m");
        println!("cargo:rustc-env=PKG_CONFIG_ALLOW_CROSS=1");
    }
}
