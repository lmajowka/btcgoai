# Bitcoin Rust AI

A high-performance Bitcoin private key scanner and puzzle solver written in Rust.

## Features

- Memory-efficient algorithms optimized for performance
- SIMD-friendly operations for vector processing
- Batch processing for improved cache locality
- Parallel execution with Rayon
- Mimalloc allocator for better memory management
- Bitcoin Puzzle TX challenge feature

## Build & Run

### Prerequisites

- Rust toolchain (stable)
- Cargo

### Building

```bash
# Debug build
cargo build

# Release build (recommended for performance)
cargo build --release
```

### Running

```bash
# Running with default settings
./target/release/btcrustai

# Show help
./target/release/btcrustai --help
```

## Bitcoin Puzzle Mode

The application includes a Bitcoin Puzzle TX mode, which focuses on trying to solve specifically targeted Bitcoin puzzles from privatekeys.pw challenges.

```bash
# Run in puzzle mode
./target/release/btcrustai --mode puzzle
```

## Performance Optimizations

This implementation includes several performance optimizations:

- Uses Rayon for parallel processing
- Mimalloc memory allocator for better memory performance
- Fat LTO (Link Time Optimization)
- Batched key processing for better cache locality
- Optimized byte comparison functions

## License

MIT
