# Bitcoin Rust AI

A high-performance Bitcoin private key scanner and puzzle solver written in Rust.

## Features

- Memory-efficient algorithms optimized for performance
- SIMD-friendly operations for vector processing
- Batch processing for improved cache locality
- Parallel execution with Rayon
- Mimalloc allocator for better memory management
- Bitcoin Puzzle TX challenge feature
- Real-time statistics and progress monitoring
- Automatic hardware detection and resource optimization
- Multi-threaded design with efficient thread synchronization
- Optimized cryptographic operations via Rust Bitcoin libraries

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

## Operation Modes

The program offers three distinct operation modes:

### 1. Normal Mode (Default)

This is the main mode for trying to find private keys from real [Bitcoin Puzzle TX Challenge](https://privatekeys.pw/puzzles/bitcoin-puzzle-tx) puzzles. In this mode:
1. All unsolved puzzles (71-160) are presented
2. You select a specific puzzle
3. The program starts the effective search for the private key using all available cores

### 2. Training Mode

Uses very low difficulty puzzles (5, 15, and 22 bits) that can be solved in seconds or minutes to verify the correct operation of the search algorithm. These puzzles have known private keys, ideal for:
- Testing the program installation
- Verifying that the search algorithm is working correctly
- Demonstrating the key discovery process

```bash
# Run in training mode
./target/release/btcrustai --mode training
```

### 3. Range Test Mode

This mode allows you to explore the search ranges of unsolved real puzzles, but without starting the actual search. It's useful for:
- Analyzing the difficulty of each puzzle
- Seeing estimates of time for complete search
- Examining private key ranges
- Getting statistics on success probabilities

```bash
# Run in range test mode
./target/release/btcrustai --mode range
```

This mode is recommended for understanding the magnitude of the challenge before attempting the real search in Normal Mode.

## Bitcoin Puzzle Mode

The application includes a Bitcoin Puzzle TX mode, which focuses on trying to solve specifically targeted Bitcoin puzzles from privatekeys.pw challenges.

```bash
# Run in puzzle mode
./target/release/btcrustai --mode puzzle
```

## Resource Detection and Optimization

The program automatically analyzes system resources to optimize performance:

1. **Hardware detection**:
   - Number of physical cores and logical CPU threads
   - Total and available memory
   - Available SIMD instruction sets (AVX2, AVX, SSE)
   - CPU brand and model

2. **Dynamic adjustment**:
   - Resource usage control (configurable percentage)
   - Optimized batch size based on available memory
   - Efficient work distribution among threads
   - Custom performance estimates for specific hardware

3. **Real-time monitoring**:
   - Global count of keys verified
   - Instantaneous and average speed
   - Progress statistics

The user can choose the percentage of system resources they want to use (10-100%), allowing other tasks to be performed while the search runs in the background.

## Performance Optimizations

This implementation includes several performance optimizations:

- Uses Rayon for parallel processing
- Mimalloc memory allocator for better memory performance
- Fat LTO (Link Time Optimization)
- Batched key processing for better cache locality
- Optimized byte comparison functions
- Zero-copy operations where possible
- Compiler optimizations with aggressive settings

## License

MIT
