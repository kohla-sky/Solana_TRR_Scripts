# Macro Analysis Tool

This tool analyzes Rust source files to determine the depth of macro usage, particularly focusing on derive and attribute macros.

## Prerequisites

- Rust (with nightly toolchain installed)
- Cargo

## Installation

Make sure you have the nightly toolchain installed:

```bash
rustup install nightly
```

## Usage

The tool can analyze either a single Rust file or an entire directory containing Rust files.

### Analyzing a Single File

```bash
cargo run -- -t path/to/your/file.rs
```

### Analyzing a Directory

```bash
cargo run -- -t path/to/your/directory -d
```

### Command Line Arguments

- `-t, --target`: Path to the Rust source file or directory to analyze
- `-d, --directory`: Flag to indicate if the target is a directory

## Output

The tool will output the macro depth for each analyzed file. The results will show:
- For single files: The macro depth of the specified file
- For directories: A sorted list of all Rust files and their respective macro depths, with the highest depth first

## Example Output

```
Analyzing 5 Rust files...
src/lib.rs: depth 2
src/models.rs: depth 1
src/utils.rs: depth 0

Results (sorted by macro depth):
--------------------------------
src/lib.rs: 2
src/models.rs: 1
```

## What is Macro Depth?

The macro depth indicates how deeply nested the macros are in your Rust code. The tool specifically looks for:
- Derive macros (`#[derive(...)]`)
- Derivative macros
- Error attribute macros
- Nested macro combinations

A higher depth number indicates more complex macro usage in your code. 