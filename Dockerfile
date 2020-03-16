FROM rust:1.42-buster

RUN /usr/bin/dpkg --add-architecture armhf
RUN apt-get update && apt-get install -y pkg-config \
	gcc-arm-linux-gnueabihf \
	g++-arm-linux-gnueabihf \
	jq

RUN rustup target add arm-unknown-linux-gnueabihf

# Build plato
WORKDIR /plato

ADD . /plato

# Plato requires a specific version of the mupdf dev library for src/mupdf_wrapper
RUN cd /plato/thirdparty && ./download.sh mupdf

CMD ["./build.sh"]
