# Setup

The OS used on the *Kobo* devices is *Linaro 2011.07*.

In order to build for this OS / architecture you can, for example, install *Ubuntu LTS 12.04* (the *GLIBC* version must be old enough) in a VM and install the following package: `gcc-4.6-arm-linux-gnueabihf`.

Install the appropriate target:
```sh
rustup target add arm-unknown-linux-gnueabihf
```

Append this:
```toml
[target.arm-unknown-linux-gnueabihf]
linker = "arm-linux-gnueabihf-gcc"
rustflags = ["-C", "target-feature=+v7,+vfp3,+a9,+neon"]
```
to `~/.cargo/config`.

Supposing that you are running Ubuntu 12.04.5 i386, you can build plato this way:
```sh
#install needed tools
sudo apt-get upgrade
sudo apt-get install gcc-arm-linux-gnueabihf g++-arm-linux-gnueabihf git build-essential libtool automake cmake ragel pck-config

#download plato from git
git clone http://github.com/traycold/plato.git
cd plato

#download thirdparty dependencies
wget -qO- http://www.bzip.org/1.0.6/bzip2-1.0.6.tar.gz | tar xz --strip-components=1 -C thirdparty/bzip2 
wget -qO- https://downloads.sourceforge.net/project/djvu/DjVuLibre/3.5.27/djvulibre-3.5.27.tar.gz | tar xz --strip-components=1 -C thirdparty/djvulibre 
wget -qO- https://zlib.net/zlib-1.2.11.tar.gz | tar xz --strip-components=1 -C thirdparty/zlib 
wget -qO- https://downloads.sourceforge.net/project/libpng/libpng16/1.6.34/libpng-1.6.34.tar.gz | tar xz --strip-components=1 -C thirdparty/libpng
wget -qO- http://www.ijg.org/files/jpegsrc.v9b.tar.gz | tar xz --strip-components=1 -C thirdparty/libjpeg
wget -qO- https://github.com/ArtifexSoftware/ghostpdl-downloads/releases/download/gs922/jbig2dec-0.14.tar.gz | tar xz --strip-components=1 -C thirdparty/jbig2dec
wget -qO- -O tmp.zip https://github.com/uclouvain/openjpeg/archive/3ed5858902055d3500a6ab183f1395686921d026.zip && unzip tmp.zip && rm tmp.zip
mv openjpeg-3ed5858902055d3500a6ab183f1395686921d026/* thirdparty/openjpeg && rm -rf openjpeg-3ed5858902055d3500a6ab183f1395686921d026
wget -qO- https://download.savannah.gnu.org/releases/freetype/freetype-2.9.tar.gz | tar xz --strip-components=1 -C thirdparty/freetype2 
wget -qO- https://download.savannah.gnu.org/releases/freetype/freetype-2.9.tar.gz | tar xz --strip-components=1 -C thirdparty/freetype2-hb
wget -qO- https://github.com/harfbuzz/harfbuzz/archive/1.7.5.tar.gz | tar xz --strip-components=1 -C thirdparty/harfbuzz 
wget -qO- https://mupdf.com/downloads/archive/mupdf-1.12.0-source.tar.gz | tar xz --strip-components=1 -C thirdparty/mupdf 

#build thirdparty deps
cd thirdparty

cd bzip2
./build-kobo.sh
ln -s libbz2.so.1.0 libbz2.so
cd ..

cd djvulibre
./build-kobo.sh
cd ..

cd zlib
./build-kobo.sh
cd ..

cd libpng
./build-kobo.sh
cd ..

cd libjpeg
./build-kobo.sh
cd ..

cd jbig2dec
./build-kobo.sh
cd ..

cd openjpeg
./build-kobo.sh
cd ..

cd freetype2
./build-kobo.sh no
cd ..

cd harfbuzz
patch < configure.patch
./build-kobo.sh
cd ..

cd freetype2-hb
./build-kobo.sh
cd ..

cd mupdf
patch < patch.diff
sed '/OPJ_STATIC$/d' -i source/fitz/load-jpx.c
./build-kobo.sh
./make-shared-lib.sh
cd ../..


#make wrapper
cd src/wrapper
./build-kobo.sh
cd ../..

#copy thirdparty libs into libs folder
rm -rf libs
mkdir libs
find thirdparty -name *.so -exec cp {} libs \;
cp src/wrapper/*.so libs

#make plato
cargo clean
./build.sh

#create dist folder (note: fonts are missing)
./dist.sh
```

You can tell what features are supported by your device from the output of `cat /proc/cpuinfo`.
