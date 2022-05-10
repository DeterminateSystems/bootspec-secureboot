#!/bin/sh

cd "$(dirname "$0")"

rm -rf ./builds
mkdir ./builds

(for system in ./*.nix; do
    printf "%s --out-link ./builds/%s\n" "$system" "$(basename -s ".nix" "$system")"
done) | xargs --max-procs=$(nproc) --max-lines=1 nix-build

rm -rf generated-synthesis
mkdir generated-synthesis
for out in ./builds/*; do
    cargo run -- "$out" "./generated-synthesis/$(basename "$out")"
done

diff -r ./expected-synthesis ./generated-synthesis
