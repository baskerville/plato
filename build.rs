fn main() {
    println!("cargo:rustc-flags=-L libs -l jpeg -l openjp2 -l jbig2dec -l lcms2 -l bz2 -l z -l m");
}
