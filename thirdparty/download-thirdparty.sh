wget -qO- http://www.bzip.org/1.0.6/bzip2-1.0.6.tar.gz | tar xz --strip-components=1 -C bzip2 
wget -qO- https://downloads.sourceforge.net/project/djvu/DjVuLibre/3.5.27/djvulibre-3.5.27.tar.gz | tar xz --strip-components=1 -C djvulibre 
wget -qO- https://zlib.net/zlib-1.2.11.tar.gz | tar xz --strip-components=1 -C zlib 
wget -qO- https://downloads.sourceforge.net/project/libpng/libpng16/1.6.34/libpng-1.6.34.tar.gz | tar xz --strip-components=1 -C libpng
wget -qO- https://downloads.sourceforge.net/project/libjpeg/libjpeg/6b/jpegsrc.v6b.tar.gz | tar xz --strip-components=1 -C libjpeg
wget -qO- https://github.com/ArtifexSoftware/ghostpdl-downloads/releases/download/gs922/jbig2dec-0.14.tar.gz | tar xz --strip-components=1 -C jbig2dec
wget -qO- -O tmp.zip https://github.com/uclouvain/openjpeg/archive/3ed5858902055d3500a6ab183f1395686921d026.zip && unzip tmp.zip && rm tmp.zip
mv openjpeg-3ed5858902055d3500a6ab183f1395686921d026/* openjpeg && rm -rf openjpeg-3ed5858902055d3500a6ab183f1395686921d026
wget -qO- https://download.savannah.gnu.org/releases/freetype/freetype-2.9.tar.gz | tar xz --strip-components=1 -C freetype2 
wget -qO- https://github.com/harfbuzz/harfbuzz/archive/1.7.6.tar.gz | tar xz --strip-components=1 -C harfbuzz 