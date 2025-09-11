# MSCD Analyzer

A Rust tool that analyzes codebases to find the maximum depth of struct compositions. Perfect for understanding how deeply nested your data structures are.

## What it does

- Calculates how many levels deep structs are nested
- Shows all struct dependencies 
- Works with complex Rust patterns like generics, modules, and type aliases
- Can analyze any Git repository directly from a URL

## Quick Start

```bash
# Build the tool
cargo build

# Analyze a local directory
cargo run -- ./src

# Analyze a GitHub repository
cargo run -- --repo https://github.com/user/project.git src/

# Get help
cargo run -- --help
```

## Real-world example

We tested this on the [Drift Protocol v2](https://github.com/drift-labs/protocol-v2) (a complex Solana DeFi protocol):

```bash
cargo run -- --repo https://github.com/drift-labs/protocol-v2.git programs/drift/src
```

**Results**: 223 structs with maximum depth of 5 levels.

## Use

This tool understands:

- **Nested modules** - finds structs buried inside `mod a { mod b { struct Inner; } }`
- **Tuple structs** - recognizes `struct Mid(Inner)` dependencies  
- **Wrapper types** - sees through `Vec<T>`, `Option<T>`, `Box<T>`, references, arrays
- **Module paths** - resolves `a::b::Inner` correctly
- **Type aliases** - follows `type T = Inner` to the real type
- **Git repositories** - clone and analyze any public repo

## Usage examples

```bash
# Local analysis
cargo run -- ./my-rust-project/src
cargo run -- /path/to/rust/files

# Remote repository analysis  
cargo run -- --repo https://github.com/solana-labs/solana.git programs/
cargo run -- --repo git@github.com:user/private-repo.git src/lib/

# Mix of both
cargo run -- --repo /local/repo/path ./specific/module/
```

## Sample output

```
Analysis Results:
=================
Maximum struct composition depth: 4

Struct count: 45

Structs with their field types:
============================

User
  - Pubkey
  - UserStats

ComplexStruct
  - Vec<Option<Box<Inner>>>
  - a::b::c::Nested
```

## Requirements

- Rust and Cargo
- Git (for repository cloning)

