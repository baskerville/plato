fn main() {
    println!("cargo:rustc-flags=-L libs -l harfbuzz -l freetype -l jpeg -l openjp2 -l jbig2dec -l bz2 -l z -l m");
}
