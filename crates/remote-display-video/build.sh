#! /bin/sh

deno run -A npm:wasm-pack build \
  --target=web crates/remote-display-video \
  -d ../../contrib/remote-display-webext/enc