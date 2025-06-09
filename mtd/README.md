## MTD (Maximum Trait Depth) Analyzer

The MTD analyzer examines trait hierarchies and implementations in Rust code. It helps identify complex trait relationships and calculates the maximum depth of trait inheritance chains in your codebase.

### What it Analyzes

The analyzer examines:
- Trait declarations and their inheritance relationships
- Trait implementations for types
- Nested trait hierarchies
- Multiple trait bounds and compound trait relationships

### Usage

Basic usage:
```bash
cd programs/mtd
cargo run [OPTIONS] [TARGET_DIR]
```

Available options:
- `-h, --help`: Show help message
- `-v, --verbose`: Show detailed analysis for each file, including trait and implementation counts
- `-f, --files`: Show maximum trait depth per file with individual summaries
- `-d, --dirs`: Show maximum trait depth per directory (recursive analysis)
- `-t, --target`: Show analysis for target directory only (non-recursive)
- `-o, --output`: Output results to specified file

Examples:
```bash
# Show help
cargo run -- -h

# Analyze current directory
cargo run .

# Analyze sample program with verbose output
cargo run -v ../sample-program/src

# Show per-file analysis of all programs
cargo run -f ../ 

# Show directory-level analysis
cargo run -d ../

# Save analysis results to a file
cargo run -v ../sample-program/src -o analysis_results.txt
```

### Output Information

The MTD analyzer provides comprehensive analysis with:

1. Trait Hierarchy Analysis:
   - Complete trait inheritance chains
   - Direct and indirect trait relationships
   - Multiple inheritance relationships

2. Implementation Analysis:
   - Type-to-trait implementation mapping
   - Implementation depth calculations
   - Trait bound satisfaction verification

3. Depth Calculations:
   - Maximum trait depth per type
   - Maximum trait depth per file
   - Maximum trait depth per directory
   - Global maximum trait depth

4. Summary Statistics:
   - Total trait count
   - Total implementation count
   - Per-file and per-directory metrics
   - Global analysis overview

### Output Formats

The analyzer can output results in different views:

1. File-Level View (`-f` flag):
   - Individual file summaries
   - Trait and implementation counts per file
   - Maximum depth calculations per file

2. Directory-Level View (`-d` flag):
   - Recursive directory analysis
   - Aggregated statistics per directory
   - Hierarchical depth reporting

3. Target-Only View (`-t` flag):
   - Non-recursive analysis of specified directory
   - Focused analysis of specific code sections

4. Verbose Output (`-v` flag):
   - Detailed trait relationships
   - Complete implementation chains
   - In-depth analysis explanations

### Output File Usage

You can direct the output to a file using the `-o` option, which is useful for:
- Saving analysis results for later review
- Comparing analyses over time
- Generating reports for documentation
- Processing the results with other tools
- Integration with CI/CD pipelines

### Example Analysis

For a codebase with traits like:
```rust
trait A {}
trait B: A {}
trait C: B {}

struct Type1;
impl C for Type1 {}
```

The analyzer will:
1. Identify the trait hierarchy (A <- B <- C)
2. Calculate the implementation depth for Type1 (depth = 3)
3. Report the relationships and statistics
4. Provide detailed analysis in the chosen output format
