rm -rf dist
mkdir dist
cp target/arm-unknown-linux-gnueabihf/release/plato dist
arm-linux-gnueabihf-strip -s dist/plato
cp -R libs dist
cp -R css dist
cp -R icons dist
cp -R scripts dist
