#!/bin/sh

rm -rf ./profile
mkdir ./profile

(for system in ./cases/*.nix; do
    printf "%s --out-link ./profile/%s\n" "$system" "$(basename -s ".nix" "$system")"
done) | xargs --max-procs=$(nproc) --max-lines=1 nix-build

rm -rf synthesized
mkdir synthesized
for out in ./profile/*; do
    cargo run -- "$out" "./synthesized/$(basename "$out")"
done