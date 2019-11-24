FROM rust:1.39-buster

RUN /usr/bin/dpkg --add-architecture armhf
RUN apt-get update && apt-get install -y libtool \
	automake \
	pkg-config \
	cmake \
	ragel \
	jq \
	patchelf \	
	gcc-arm-linux-gnueabihf \
	g++-arm-linux-gnueabihf \
	libfreetype6-dev:armhf \
	libglib2.0-dev:armhf \
	libcairo2-dev:armhf \
	libstdc++6:armhf

RUN rustup target add arm-unknown-linux-gnueabihf

# Build plato
WORKDIR /plato
ADD . /plato
CMD ["./build.sh"]
