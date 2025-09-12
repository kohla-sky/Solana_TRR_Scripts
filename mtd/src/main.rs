use std::collections::{HashMap, HashSet};
use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

struct TraitInfo {
    name: String,
    supertraits: Vec<String>,
}

struct ImplInfo {
    type_name: String,
    trait_name: String,
}

struct FileAnalyzer {
    traits: Vec<TraitInfo>,
    impls: Vec<ImplInfo>,
}

impl FileAnalyzer {
    fn new() -> Self {
        FileAnalyzer {
            traits: Vec::new(),
            impls: Vec::new(),
        }
    }

    fn analyze_file(&mut self, path: &Path) -> io::Result<()> {
        let content = fs::read_to_string(path)?;
        
        // Parse the entire file content, handling multiline declarations
        self.parse_content(&content);
        
        Ok(())
    }

    fn parse_content(&mut self, content: &str) {
        let mut chars = content.chars().peekable();
        let mut current_line = String::new();
        let mut in_multiline_declaration = false;
        let mut brace_depth = 0;
        let mut declaration_buffer = String::new();

        while let Some(ch) = chars.next() {
            match ch {
                '\n' | '\r' => {
                    if !in_multiline_declaration {
                        self.process_line(&current_line.trim());
                        current_line.clear();
                    } else {
                        declaration_buffer.push(' ');
                    }
                }
                '{' => {
                    current_line.push(ch);
                    if in_multiline_declaration {
                        declaration_buffer.push(ch);
                        brace_depth += 1;
                        if brace_depth == 1 {
                            // End of declaration, process it
                            self.process_line(&declaration_buffer.trim());
                            declaration_buffer.clear();
                            in_multiline_declaration = false;
                            brace_depth = 0;
                        }
                    }
                }
                '}' => {
                    current_line.push(ch);
                    if in_multiline_declaration && brace_depth > 0 {
                        declaration_buffer.push(ch);
                        brace_depth -= 1;
                    }
                }
                _ => {
                    current_line.push(ch);
                    if in_multiline_declaration {
                        declaration_buffer.push(ch);
                    }
                }
            }

            // Check if we're starting a multiline declaration
            if !in_multiline_declaration && (
                self.is_trait_declaration_start(&current_line) || 
                self.is_impl_declaration_start(&current_line)
            ) {
                // Check if the line ends without opening brace - might be multiline
                let trimmed = current_line.trim();
                if !trimmed.contains('{') && !trimmed.ends_with(';') {
                    in_multiline_declaration = true;
                    declaration_buffer = current_line.clone();
                    current_line.clear();
                }
            }
        }

        // Process any remaining line
        if !current_line.trim().is_empty() {
            self.process_line(&current_line.trim());
        }
    }

    fn is_trait_declaration_start(&self, line: &str) -> bool {
        let trimmed = line.trim();
        // Handle all visibility modifiers and unsafe
        trimmed.starts_with("trait ") ||
        trimmed.starts_with("pub trait ") ||
        trimmed.starts_with("pub(crate) trait ") ||
        trimmed.starts_with("pub(super) trait ") ||
        trimmed.starts_with("pub(self) trait ") ||
        trimmed.starts_with("pub(in ") && trimmed.contains(") trait ") ||
        trimmed.starts_with("unsafe trait ") ||
        trimmed.starts_with("pub unsafe trait ") ||
        trimmed.starts_with("pub(crate) unsafe trait ") ||
        trimmed.starts_with("pub(super) unsafe trait ")
    }

    fn is_impl_declaration_start(&self, line: &str) -> bool {
        let trimmed = line.trim();
        trimmed.starts_with("impl ") ||
        trimmed.starts_with("unsafe impl ")
    }

    fn process_line(&mut self, line: &str) {
        if self.is_trait_declaration_start(line) {
            if let Some(trait_info) = self.parse_trait_declaration(line) {
                self.traits.push(trait_info);
            }
        } else if self.is_impl_declaration_start(line) {
            if let Some(impl_info) = self.parse_impl_declaration(line) {
                self.impls.push(impl_info);
            }
        }
    }

    fn parse_trait_declaration(&self, line: &str) -> Option<TraitInfo> {
        // Remove all visibility and safety modifiers
        let mut cleaned = line.trim();
        
        // Remove visibility modifiers
        if cleaned.starts_with("pub(") {
            if let Some(end_paren) = cleaned.find(')') {
                cleaned = &cleaned[end_paren + 1..].trim();
            }
        } else if cleaned.starts_with("pub ") {
            cleaned = &cleaned[4..];
        }
        
        // Remove unsafe modifier
        if cleaned.starts_with("unsafe ") {
            cleaned = &cleaned[7..];
        }
        
        // Remove trait keyword
        if cleaned.starts_with("trait ") {
            cleaned = &cleaned[6..];
        } else {
            return None;
        }

        // Find the trait name and supertraits
        let colon_pos = cleaned.find(':');
        let brace_pos = cleaned.find('{');
        
        let name_end = match (colon_pos, brace_pos) {
            (Some(colon), Some(brace)) => colon.min(brace),
            (Some(colon), None) => colon,
            (None, Some(brace)) => brace,
            (None, None) => cleaned.len(),
        };

        let name = cleaned[..name_end].trim().to_string();
        if name.is_empty() {
            return None;
        }

        let supertraits = if let Some(colon_pos) = colon_pos {
            let supertrait_part = if let Some(brace_pos) = brace_pos {
                &cleaned[colon_pos + 1..brace_pos]
            } else {
                &cleaned[colon_pos + 1..]
            };
            
            supertrait_part
                .split('+')
                .map(|s| self.clean_identifier(s.trim()))
                .filter(|s| !s.is_empty())
                .collect()
        } else {
            Vec::new()
        };

        Some(TraitInfo {
            name: self.clean_identifier(&name),
            supertraits,
        })
    }

    fn parse_impl_declaration(&self, line: &str) -> Option<ImplInfo> {
        let mut cleaned = line.trim();
        
        // Remove unsafe modifier
        if cleaned.starts_with("unsafe ") {
            cleaned = &cleaned[7..];
        }
        
        // Remove impl keyword
        if cleaned.starts_with("impl ") {
            cleaned = &cleaned[5..];
        } else {
            return None;
        }

        // Handle cases like "impl Trait for Type"
        if let Some(for_idx) = cleaned.find(" for ") {
            let trait_part = &cleaned[..for_idx];
            let type_part = &cleaned[for_idx + 5..];
            
            let trait_name = self.clean_identifier(trait_part.trim());
            let type_name = self.clean_identifier(type_part.trim());
            
            if !trait_name.is_empty() && !type_name.is_empty() {
                return Some(ImplInfo {
                    type_name,
                    trait_name,
                });
            }
        }
        
        None
    }

    fn clean_identifier(&self, identifier: &str) -> String {
        identifier
            .trim()
            .trim_end_matches('{')
            .trim_end_matches('}')
            .trim()
            .to_string()
    }
}

struct TraitAnalyzer {
    trait_graph: HashMap<String, Vec<String>>,
    impl_map: HashMap<String, HashSet<String>>,
}

impl TraitAnalyzer {
    fn new() -> Self {
        TraitAnalyzer {
            trait_graph: HashMap::new(),
            impl_map: HashMap::new(),
        }
    }

    fn add_file_analysis(&mut self, file_analyzer: &FileAnalyzer) {
        // Add traits to graph
        for trait_info in &file_analyzer.traits {
            self.trait_graph.insert(
                trait_info.name.clone(),
                trait_info.supertraits.clone(),
            );
        }

        // Add implementations
        for impl_info in &file_analyzer.impls {
            self.impl_map
                .entry(impl_info.type_name.clone())
                .or_insert_with(|| HashSet::new())
                .insert(impl_info.trait_name.clone());
        }
    }

    fn calculate_max_depth(&self, type_name: &str) -> usize {
        let mut visited = HashSet::new();
        let mut max_depth = 0;

        if let Some(traits) = self.impl_map.get(type_name) {
            for trait_name in traits {
                let depth = self.dfs_trait_depth(trait_name, &mut visited);
                max_depth = max_depth.max(depth);
            }
        }

        max_depth
    }

    fn dfs_trait_depth(&self, trait_name: &str, visited: &mut HashSet<String>) -> usize {
        if !visited.insert(trait_name.to_string()) {
            return 0;
        }

        let mut max_depth = 0;
        if let Some(supertraits) = self.trait_graph.get(trait_name) {
            for supertrait in supertraits {
                let depth = self.dfs_trait_depth(supertrait, visited);
                max_depth = max_depth.max(depth);
            }
        }

        visited.remove(trait_name);
        max_depth + 1
    }

    fn get_summary(&self) -> AnalysisSummary {
        let mut max_depth = 0;
        for (type_name, _) in &self.impl_map {
            max_depth = max_depth.max(self.calculate_max_depth(type_name));
        }
        
        AnalysisSummary {
            max_depth,
            trait_count: self.trait_graph.len(),
            impl_count: self.impl_map.len(),
        }
    }
}

struct AnalysisSummary {
    max_depth: usize,
    trait_count: usize,
    impl_count: usize,
}

fn visit_dirs(dir: &Path, cb: &mut dyn FnMut(&Path), recursive: bool) -> io::Result<()> {
    if dir.is_dir() {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                if recursive {
                    visit_dirs(&path, cb, recursive)?;
                }
            } else if path.extension().map_or(false, |ext| ext == "rs") {
                cb(&path);
            }
        }
    }
    Ok(())
}

fn print_help() {
    println!("Usage: {} [OPTIONS] [TARGET_DIR]", env::args().next().unwrap());
    println!("Options:");
    println!("  -h, --help     Show this help message");
    println!("  -v, --verbose  Show detailed analysis for each file");
    println!("  -f, --files    Show maximum trait depth per file");
    println!("  -d, --dirs     Show maximum trait depth per directory (recursive)");
    println!("  -t, --target   Show analysis for target directory only (non-recursive)");
    println!();
    println!("If TARGET_DIR is not specified, the current directory will be used.");
}

fn main() -> io::Result<()> {
    let args: Vec<String> = env::args().collect();
    let mut verbose = false;
    let mut show_per_file = false;
    let mut show_per_dir = false;
    let mut target_only = false;
    let mut target_dir = None;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "-h" | "--help" => {
                print_help();
                return Ok(());
            }
            "-v" | "--verbose" => verbose = true,
            "-f" | "--files" => show_per_file = true,
            "-d" | "--dirs" => show_per_dir = true,
            "-t" | "--target" => target_only = true,
            dir if !dir.starts_with('-') => {
                target_dir = Some(PathBuf::from(dir));
            }
            _ => {
                eprintln!("Unknown option: {}", args[i]);
                print_help();
                return Ok(());
            }
        }
        i += 1;
    }

    let target_dir = target_dir.unwrap_or_else(|| PathBuf::from("."));
    if !target_dir.exists() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("Directory '{}' does not exist", target_dir.display()),
        ));
    }

    println!("Analyzing Rust files in directory: {}", target_dir.display());
    if target_only {
        println!("(Non-recursive analysis)");
    }
    
    let mut file_summaries = HashMap::new();
    let mut dir_summaries = HashMap::new();
    let mut trait_analyzer = TraitAnalyzer::new();

    // Collect file-level and directory-level data
    visit_dirs(&target_dir, &mut |path: &Path| {
        let mut file_analyzer = FileAnalyzer::new();
        match file_analyzer.analyze_file(path) {
            Ok(()) => {
                if verbose {
                    println!("\nAnalyzing file: {}", path.display());
                    println!("Found {} traits and {} implementations", 
                        file_analyzer.traits.len(),
                        file_analyzer.impls.len());
                }

                // Create a separate analyzer for this file
                if show_per_file {
                    let mut single_file_analyzer = TraitAnalyzer::new();
                    single_file_analyzer.add_file_analysis(&file_analyzer);
                    let summary = single_file_analyzer.get_summary();
                    file_summaries.insert(path.to_path_buf(), summary);
                }

                // Add to directory summary
                if show_per_dir || target_only {
                    let dir_path = path.parent().unwrap_or(Path::new("")).to_path_buf();
                    let dir_analyzer = dir_summaries
                        .entry(dir_path)
                        .or_insert_with(TraitAnalyzer::new);
                    dir_analyzer.add_file_analysis(&file_analyzer);
                }

                // Add to global analyzer
                trait_analyzer.add_file_analysis(&file_analyzer);
            }
            Err(e) => {
                eprintln!("Error analyzing {}: {}", path.display(), e);
            }
        }
    }, !target_only)?;

    // Print file-level summaries if requested
    if show_per_file {
        println!("\nFile-Level Summary:");
        println!("==================");
        for (path, summary) in &file_summaries {
            println!("\n{}", path.display());
            println!("  Maximum Trait Depth: {}", summary.max_depth);
            println!("  Trait Count: {}", summary.trait_count);
            println!("  Implementation Count: {}", summary.impl_count);
        }
    }

    // Print directory-level summaries if requested
    if show_per_dir {
        println!("\nDirectory-Level Summary (Recursive):");
        println!("=================================");
        for (dir_path, analyzer) in &dir_summaries {
            let summary = analyzer.get_summary();
            println!("\n{}", dir_path.display());
            println!("  Maximum Trait Depth: {}", summary.max_depth);
            println!("  Trait Count: {}", summary.trait_count);
            println!("  Implementation Count: {}", summary.impl_count);
        }
    }

    // Print target directory summary if requested
    if target_only {
        println!("\nTarget Directory Summary:");
        println!("=======================");
        if let Some(analyzer) = dir_summaries.get(&target_dir) {
            let summary = analyzer.get_summary();
            println!("  Maximum Trait Depth: {}", summary.max_depth);
            println!("  Trait Count: {}", summary.trait_count);
            println!("  Implementation Count: {}", summary.impl_count);
        } else {
            println!("No Rust files found in target directory");
        }
    }

    // Print global summary
    let global_summary = trait_analyzer.get_summary();
    println!("\nGlobal Summary:");
    println!("==============");
    println!("Overall Maximum Trait Depth: {}", global_summary.max_depth);
    println!("Total Trait Count: {}", global_summary.trait_count);
    println!("Total Implementation Count: {}", global_summary.impl_count);

    // Print trait hierarchy if no specific summary was requested
    if !show_per_file && !show_per_dir && !target_only {
        println!("\nTrait Hierarchy:");
        for (trait_name, supertraits) in &trait_analyzer.trait_graph {
            println!("{} -> {:?}", trait_name, supertraits);
        }

        println!("\nType Implementations and Maximum Trait Depth:");
        for (type_name, traits) in &trait_analyzer.impl_map {
            println!("\n{} implements:", type_name);
            for trait_name in traits {
                println!("  - {}", trait_name);
            }
            let depth = trait_analyzer.calculate_max_depth(type_name);
            println!("Maximum trait depth: {}", depth);
        }
    }

    Ok(())
} 
