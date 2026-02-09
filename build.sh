#!/bin/bash
# Ultimate File Recovery v11.5 - Build Script

set -e

echo "Ultimate File Recovery v11.5 - Build"
echo "========================================"
echo ""

# Check Rust
if ! command -v cargo &> /dev/null; then
    echo "Rust not found. Installing..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source $HOME/.cargo/env
fi

echo "Rust: $(rustc --version)"
echo ""

# Install Python dependencies
echo "Installing Python dependencies..."
# pip3 install -q rich || sudo pip3 install -q rich
echo "Python dependencies installed"
echo ""

# Build Rust accelerator
echo "Building Rust accelerator..."
cd accelerator
cargo build --release
cd ..

# Ensure lib/ directory exists
mkdir -p lib

# Copy library to lib/
if [ -f "accelerator/target/release/librust_accelerator.so" ]; then
    cp accelerator/target/release/librust_accelerator.so lib/rust_accelerator.so
    echo "Rust accelerator built: lib/rust_accelerator.so"
elif [ -f "accelerator/target/release/librust_accelerator.dylib" ]; then
    cp accelerator/target/release/librust_accelerator.dylib lib/rust_accelerator.so
    echo "Rust accelerator built: lib/rust_accelerator.so (from .dylib)"
else
    echo "Build failed: library not found"
    exit 1
fi

echo ""
echo "========================================"
echo "BUILD COMPLETE"
echo "========================================"
echo ""
echo "Usage:"
echo "  python3 recover.py disk.img --target-size-min 15 --target-size-max 300 -o output/"
echo ""
