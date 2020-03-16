FROM rust:1.42-buster

RUN /usr/bin/dpkg --add-architecture armhf
RUN apt-get update && apt-get install -y pkg-config \
	jq

RUN wget -q --show-progress -O "armf.tar.xz" "https://releases.linaro.org/components/toolchain/binaries/4.9-2017.01/arm-linux-gnueabihf/gcc-linaro-4.9.4-2017.01-x86_64_arm-linux-gnueabihf.tar.xz"
RUN mkdir -p /opt/armf && tar -x --strip-components 1 -C "/opt/armf" -f "armf.tar.xz" && rm "armf.tar.xz"
RUN find /opt/armf/bin -name 'arm-*' -exec ln -s {} /usr/bin \;

RUN rustup target add arm-unknown-linux-gnueabihf

# Build plato
WORKDIR /plato

ADD . /plato

# Plato requires a specific version of the mupdf dev library for src/mupdf_wrapper
RUN cd /plato/thirdparty && ./download.sh mupdf

CMD ["./build.sh"]
