# Rust Recovery - Stage 1 Implementation

Ultimate File Recovery - Rust Implementation (Stage 1)

## Overview

This is the Stage 1 implementation of the Rust-based file recovery tool, featuring:

- **Zero-copy memory mapping** with `memmap2` and `Arc<Mmap>`
- **Type-safe primitives** with newtype wrappers (`Offset`, `Size`, `ClusterId`)
- **Robust error handling** with `thiserror` and custom `RecoveryError` type
- **Comprehensive CLI** mirroring Python `recover.py` arguments
- **Bounds-checked disk access** with `FragmentSlice` and `DiskImage::get_slice()`

## Architecture

### Module Structure

```
rust-recovery/
├── src/
│   ├── main.rs         # Entry point and orchestration
│   ├── cli.rs          # Command-line argument parsing (clap)
│   ├── disk.rs         # Zero-copy disk image access
│   ├── types.rs        # Newtype wrappers (Offset, Size, ClusterId)
│   └── error.rs        # Error types and Result alias
├── Cargo.toml
└── README.md
```

### Key Components

#### 1. **disk.rs** - Zero-Copy Memory Mapping

- `DiskImage`: Wraps `Arc<Mmap>` for shared, zero-copy access to disk images
- `FragmentSlice<'a>`: Borrowed slice with offset metadata
- `get_slice()`: Bounds-checked slice extraction with comprehensive error handling

```rust
let disk = DiskImage::open("image.img")?;
let slice = disk.get_slice(Offset::new(0), 512)?;
// slice.data contains zero-copy reference to mmap'd data
```

#### 2. **types.rs** - Type-Safe Primitives

Newtype wrappers prevent mixing incompatible units:

- `Offset(u64)`: Byte offsets in disk images
- `Size(u64)`: Sizes in bytes
- `ClusterId(u64)`: Cluster identifiers

All types implement `Display` for debugging and provide checked arithmetic.

#### 3. **error.rs** - Comprehensive Error Handling

- `RecoveryError` enum with `thiserror` for ergonomic error messages
- Specialized variants for I/O, mmap, bounds checks, and validation
- `Result<T>` type alias for consistency

#### 4. **cli.rs** - Command-Line Interface

Full feature parity with Python `recover.py`:

- Image path (required)
- `--target-size-min/max` (KB, default: 15-300)
- `--chunk-min/max` (KB, default: 32-2048)
- `--reverse`: Reverse scan mode
- `--nvme`: NVMe optimization
- `--early-exit N`: Stop after N files
- `--output DIR`: Output directory
- `--enable-exfat`: Enable exFAT scanning
- `--full-exfat-recovery`: FAT chain following (default: true)
- `--no-live`: Disable live dashboard
- `--links-only`: Extract links only
- `--semantic-scan`: Semantic analysis

Includes validation logic and helper methods for unit conversion.

## Building

```bash
cargo build          # Debug build
cargo build --release # Release build (optimized)
```

## Running

```bash
# Basic usage
./target/debug/rust-recovery image.img

# With options
./target/debug/rust-recovery image.img \
  --target-size-min 10 \
  --target-size-max 500 \
  --reverse \
  --nvme \
  --enable-exfat \
  --output recovery_output

# Help
./target/debug/rust-recovery --help
```

## Testing

```bash
cargo test
```

Current test coverage:
- CLI argument validation and byte conversions
- Fragment slice creation and size calculation
- Offset arithmetic with overflow checks

## Stage 1 Deliverables ✅

- ✅ Binary Cargo project created at `rust-recovery/`
- ✅ Module structure: `disk`, `types`, `error`, `cli`
- ✅ Zero-copy `memmap2` core with `Arc<Mmap>`
- ✅ `FragmentSlice<'a>` with offset and data
- ✅ `DiskImage::get_slice()` with comprehensive bounds checks
- ✅ Newtype wrappers: `Offset`, `Size`, `ClusterId`
- ✅ `RecoveryError` via `thiserror` + `Result` alias
- ✅ `clap` CLI mirroring `recover.py` arguments and defaults
- ✅ Main.rs wired to parse args and open disk image
- ✅ `--full-exfat-recovery` default behavior preserved (true)
- ✅ `cargo build` passes without errors

## Next Stages (Not Yet Implemented)

Future stages will add:

- **Stage 2**: Pattern scanning and matching engine
- **Stage 3**: Candidate discovery and clustering
- **Stage 4**: Fragment assembly and stream solving
- **Stage 5**: File reconstruction and validation
- **Stage 6**: Report generation (HTML/JSON)

## Design Notes

### Zero-Copy Architecture

The `Arc<Mmap>` design enables:
- Shared ownership across threads (for future parallel scanning)
- Zero-copy slicing via `FragmentSlice` borrows
- Automatic cleanup when all references drop

### Safety

- All disk access goes through bounds-checked `get_slice()`
- Offset arithmetic uses checked operations to prevent overflow
- Memory mapping is `unsafe` but encapsulated in safe API

### Performance Considerations

- Memory mapping avoids unnecessary copies
- `Arc` enables cheap cloning for multi-threaded access
- Future stages will add parallel scanning via `rayon`

## Dependencies

- `clap`: CLI parsing with derive macros
- `memmap2`: Memory-mapped file I/O
- `thiserror`: Ergonomic error handling
- `anyhow`: Additional error utilities

## License

Part of Ultimate File Recovery project.
