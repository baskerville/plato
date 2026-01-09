.PHONY: all build clean

all: build

fragment-generation-utils.bundle.mjs:
	wget https://esm.sh/text-fragments-polyfill@6.4.1/es2022/dist/fragment-generation-utils.bundle.mjs -O fragment-generation-utils.bundle.mjs

build: fragment-generation-utils.bundle.mjs
	zip -r -FS ./webext.zip * --exclude webext.zip

clean:
	rm -f webext.zip fragment-generation-utils.bundle.mjs
