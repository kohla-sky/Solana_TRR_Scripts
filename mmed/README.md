# Macro Analysis Tool

This tool provides advanced analysis of Rust source files to determine the depth and complexity of macro usage, with particular focus on derive macros, attribute macros, and other macro patterns.

## Prerequisites

- Rust (with nightly toolchain installed)
- Cargo

## Installation

Make sure you have the nightly toolchain installed:

```bash
rustup install nightly
```

## Usage

The tool analyzes Rust source files in a directory, providing detailed analysis of macro usage and potential complexity warnings.

```bash
cargo run -- --dir path/to/your/directory
```

### Command Line Arguments

- `--dir, -d`: Path to the directory containing Rust files to analyze

## Analysis Features

The tool performs comprehensive macro analysis including:

1. **Macro Nesting Depth**
   - Tracks the maximum nesting level of macros
   - Identifies complex macro hierarchies

2. **Procedural Macro Detection**
   - Recognizes common proc macros (derive, anchor_lang, serde, etc.)
   - Estimates actual macro expansion depth

3. **Pattern Recognition**
   - Identifies macro repetition patterns (`$(...)*)`)
   - Detects compiler helper macros
   - Analyzes string literals for potential macro calls

4. **Warning System**
   - Reports potential complexity issues
   - Identifies areas where actual macro depth might be higher than reported

## Example Output

```
File: src/example.rs
Maximum macro nesting depth: 3

Analysis warnings:
- Found proc-macro attribute 'derive' - actual macro depth may be significantly higher
- Warning: Macro 'vec!' contains repetition pattern - actual depth may be higher

Analysis Summary:
Files analyzed: 5
Maximum macro nesting depth across all files: 4

Warning Statistics:
Procedural macro 'derive': 12 instances
Compiler helper macro 'format_args': 3 instances
Macro with repetition pattern 'vec': 2 instances
```

## Understanding the Results

### Macro Depth
The macro depth indicates how deeply nested the macros are in your Rust code. The tool analyzes:
- Direct macro invocations (`macro_name!`)
- Derive macros (`#[derive(...)]`)
- Attribute macros
- Nested macro combinations

### Warning Types
1. **Procedural Macros**: Identified when known proc-macros are used (derive, anchor_lang, serde, etc.)
2. **Compiler Helpers**: Special handling for compiler-generated macros (format_args, assert, print, etc.)
3. **Repetition Patterns**: Macros using repetition syntax that might expand to deeper structures
4. **String Literal Macros**: Potential macro calls within string literals

A higher depth number or more warnings indicate more complex macro usage in your code, which might affect compilation time and code maintainability. 