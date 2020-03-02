FROM rust:1.39-buster

RUN /usr/bin/dpkg --add-architecture armhf
RUN apt-get update && apt-get install -y pkg-config \
	gcc-arm-linux-gnueabihf \
	g++-arm-linux-gnueabihf \
	jq

RUN rustup target add arm-unknown-linux-gnueabihf

# Build plato
WORKDIR /plato
ADD . /plato
CMD ["./build-kobo.sh"]
