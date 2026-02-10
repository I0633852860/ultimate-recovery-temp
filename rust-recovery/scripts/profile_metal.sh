#!/bin/bash

# Компиляция с релизом и оптимизациями
RUSTFLAGS="-C target-cpu=native" cargo build --release

# Профилирование CPU
echo "[1] Running CPU profiling..."
perf record -F 999 -g --call-graph dwarf \
    ./target/release/rust-recovery scan test_optimization.img

# Анализ hotspots
echo "[2] Analyzing hotspots..."
perf report --stdio > profile_hotspots.txt

# Профилирование cache misses
echo "[3] Running cache profiling..."
perf stat -e cache-references,cache-misses,L1-dcache-loads,L1-dcache-load-misses \
    ./target/release/rust-recovery scan test_optimization.img

# Анализ false sharing
echo "[4] Detecting false sharing..."
perf c2c record ./target/release/rust-recovery scan test_optimization.img
perf c2c report --stdio > false_sharing.txt

# Branch prediction
echo "[5] Analyzing branch predictions..."
perf stat -e branches,branch-misses \
    ./target/release/rust-recovery scan test_optimization.img

echo "Profile complete. Results:"
echo "  - profile_hotspots.txt"
echo "  - false_sharing.txt"
