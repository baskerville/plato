# rustlang images no longer support debian:stretch
FROM buildpack-deps:stretch

# From https://github.com/rust-lang/docker-rust
ENV RUSTUP_HOME=/usr/local/rustup \
    CARGO_HOME=/usr/local/cargo \
    PATH=/usr/local/cargo/bin:$PATH \
    RUST_VERSION=1.60.0

RUN set -eux; \
    dpkgArch="$(dpkg --print-architecture)"; \
    case "${dpkgArch##*-}" in \
        amd64) rustArch='x86_64-unknown-linux-gnu'; rustupSha256='ee7ade44063c96c6a37012cc599cb560dce95205c86d17b247c726d2285b230c' ;; \
        armhf) rustArch='armv7-unknown-linux-gnueabihf'; rustupSha256='afab10b89436bfb5ff7db4f4d5ad4d82faee98165915801d73e965e873661b1c' ;; \
        arm64) rustArch='aarch64-unknown-linux-gnu'; rustupSha256='99f42ab89c790e8825d91c99edee22b0176f68969d42dc27a89a5d9651c3ceba' ;; \
        i386) rustArch='i686-unknown-linux-gnu'; rustupSha256='6fefdd5c429545b9c710c5492402215e1256cb002f19840db64303281b5ded3c' ;; \
        *) echo >&2 "unsupported architecture: ${dpkgArch}"; exit 1 ;; \
    esac; \
    url="https://static.rust-lang.org/rustup/archive/1.23.0/${rustArch}/rustup-init"; \
    wget "$url"; \
    echo "${rustupSha256} *rustup-init" | sha256sum -c -; \
    chmod +x rustup-init; \
    ./rustup-init -y --no-modify-path --profile minimal --default-toolchain $RUST_VERSION --default-host ${rustArch}; \
    rm rustup-init; \
    chmod -R a+w $RUSTUP_HOME $CARGO_HOME; \
    rustup --version; \
    cargo --version; \
    rustc --version;

# plato-specific
RUN /usr/bin/dpkg --add-architecture armhf
RUN apt-get update && apt-get install -y pkg-config \
	gcc-arm-linux-gnueabihf \
	g++-arm-linux-gnueabihf \
	jq

RUN rustup target add arm-unknown-linux-gnueabihf

# Build plato
WORKDIR /plato

ADD . /plato

# Plato requires a specific version of the mupdf dev library for /mupdf_wrapper
RUN cd /plato/thirdparty && ./download.sh mupdf

CMD ["./build.sh"]
