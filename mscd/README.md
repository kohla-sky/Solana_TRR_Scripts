## MSCD Analyzer

This tool analyzes Rust codebases to determine the maximum depth of struct compositions and provides detailed information about struct relationships.

## Features

- Calculates maximum struct nesting depth
- Lists all structs and their field types
- Analyzes struct dependencies
- Provides detailed struct composition information
- Handles recursive struct definitions

## Installation & Build

### Prerequisites
- Rust and Cargo (latest stable version)
- `syn` and `quote` crates (included in Cargo.toml)

### Build Steps
1. First, build the analyzer:
```bash
cargo build
```

2. Alternatively, you can build and run in release mode for better performance:
```bash
cargo build --release
```

## Usage

### Basic Usage

To analyze a directory containing Rust files:

```bash
cargo run <directory_path>
```

For example:
```bash
cargo run ../sample-program/src    # Analyze a specific program
cargo run ../                      # Analyze all programs in parent directory
```

### Command Line Options

- No arguments or `-h` or `--help`: Shows help message
- `<directory>`: Path to the directory containing Rust files to analyze

### Output Information

The analyzer provides:
1. Maximum struct composition depth
2. Total number of structs found
3. Detailed listing of each struct and its field types
4. Struct dependency relationships

## Example Output

```
Analysis Results:
=================
Maximum struct composition depth: 4
Struct count: 140

Structs with their field types:
============================
[Detailed struct information follows...]
```

## Error Handling

- If the specified directory doesn't exist, an error message will be displayed
- Invalid Rust files are skipped with appropriate error messages
- Circular dependencies are properly handled

## Notes

- The analyzer recursively processes all `.rs` files in the specified directory
- Struct composition depth is calculated by following field type references
