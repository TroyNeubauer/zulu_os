#!/bin/bash
(cd ../../../cargo-show-asm && cargo build --release) && \
../../../cargo-show-asm/target/release/cargo-asm asm --bin zulu_os core
