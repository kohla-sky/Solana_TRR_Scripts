use std::collections::{HashMap, HashSet};
use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use syn::{visit::Visit, Macro, File, Item, parse_file};
use quote::ToTokens;

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
        
        // Simple parsing for trait declarations
        for line in content.lines() {
            let line = line.trim();
            
            // Parse trait declarations
            if line.starts_with("trait ") || line.starts_with("pub trait ") {
                if let Some(trait_info) = self.parse_trait_declaration(line) {
                    self.traits.push(trait_info);
                }
            }
            
            // Parse trait implementations
            if line.starts_with("impl ") {
                if let Some(impl_info) = self.parse_impl_declaration(line) {
                    self.impls.push(impl_info);
                }
            }
        }
        
        Ok(())
    }

    fn parse_trait_declaration(&self, line: &str) -> Option<TraitInfo> {
        let line = line.trim_start_matches("pub ").trim_start_matches("trait ");
        let mut parts = line.split(':');
        let name = parts.next()?.trim().to_string();
        
        let supertraits = if let Some(supertrait_part) = parts.next() {
            supertrait_part
                .split('+')
                .map(|s| s.trim().trim_end_matches('{').trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        } else {
            Vec::new()
        };

        Some(TraitInfo {
            name,
            supertraits,
        })
    }

    fn parse_impl_declaration(&self, line: &str) -> Option<ImplInfo> {
        let line = line.trim_start_matches("impl ");
        
        // Handle cases like "impl Trait for Type"
        if let Some(for_idx) = line.find(" for ") {
            let trait_part = &line[..for_idx];
            let type_part = &line[for_idx + 5..];
            
            let trait_name = trait_part.trim().trim_end_matches('{').to_string();
            let type_name = type_part.trim().trim_end_matches('{').to_string();
            
            return Some(ImplInfo {
                type_name,
                trait_name,
            });
        }
        
        None
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

struct MacroVisitor {
    max_depth: usize,
    current_depth: usize,
    macro_depths: HashMap<String, usize>,
}

impl MacroVisitor {
    fn new() -> Self {
        MacroVisitor {
            max_depth: 0,
            current_depth: 0,
            macro_depths: HashMap::new(),
        }
    }

    fn analyze_macro_tokens(&mut self, tokens: proc_macro2::TokenStream) {
        // Try parsing tokens as a complete file
        if let Ok(file) = syn::parse2::<File>(tokens.clone()) {
            self.visit_file(&file);
        }

        // Also try parsing as individual macros
        if let Ok(mac) = syn::parse2::<Macro>(tokens) {
            self.visit_macro(&mac);
        }
    }
}

impl<'ast> Visit<'ast> for MacroVisitor {
    fn visit_macro(&mut self, mac: &'ast Macro) {
        self.current_depth += 1;
        
        // Update max depth if current depth is greater
        self.max_depth = self.max_depth.max(self.current_depth);
        
        // Get macro name
        let name = mac.path.segments.last()
            .map(|seg| seg.ident.to_string())
            .unwrap_or_else(|| "unknown".to_string());
            
        // Update depth for this specific macro
        self.macro_depths.entry(name)
            .and_modify(|d| *d = (*d).max(self.current_depth))
            .or_insert(self.current_depth);

        // Analyze the macro's token stream for nested macros
        self.analyze_macro_tokens(mac.tokens.clone());
        
        self.current_depth -= 1;
    }

    fn visit_item(&mut self, item: &'ast Item) {
        // Check for attribute macros
        for attr in item.attrs() {
            if attr.path().is_ident("derive") || 
               attr.path().is_ident("proc_macro") ||
               attr.path().is_ident("proc_macro_derive") {
                self.current_depth += 1;
                self.max_depth = self.max_depth.max(self.current_depth);
                
                // Parse and visit the attribute's tokens
                if let Ok(tokens) = attr.parse_args::<proc_macro2::TokenStream>() {
                    self.analyze_macro_tokens(tokens);
                }
                
                self.current_depth -= 1;
            }
        }
        
        // Continue visiting the item's contents
        syn::visit::visit_item(self, item);
    }
}

fn analyze_file(path: &Path) -> io::Result<MacroVisitor> {
    let content = fs::read_to_string(path)?;
    let syntax = parse_file(&content).map_err(|e| {
        io::Error::new(io::ErrorKind::InvalidData, format!("Parse error: {}", e))
    })?;

    let mut visitor = MacroVisitor::new();
    visitor.visit_file(&syntax);
    Ok(visitor)
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
    println!("  -o, --output   Output results to specified file");
    println!();
    println!("If TARGET_DIR is not specified, the current directory will be used.");
}

fn write_output(content: String, output_file: Option<&str>) -> io::Result<()> {
    match output_file {
        Some(path) => fs::write(path, content),
        None => {
            print!("{}", content);
            Ok(())
        }
    }
}

fn main() -> io::Result<()> {
    let args: Vec<String> = env::args().collect();
    let mut verbose = false;
    let mut show_per_file = false;
    let mut show_per_dir = false;
    let mut target_only = false;
    let mut target_dir = None;
    let mut output_file = None;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "-h" | "--help" => {
                print_help();
                return Ok(());
            }
            "-v" | "--verbose" => {
                verbose = true;
            }
            "-f" | "--files" => {
                show_per_file = true;
            }
            "-d" | "--dirs" => {
                show_per_dir = true;
            }
            "-t" | "--target" => {
                target_only = true;
            }
            "-o" | "--output" => {
                if i + 1 < args.len() {
                    output_file = Some(args[i + 1].clone());
                    i += 1;
                } else {
                    eprintln!("Error: Output file path not provided");
                    print_help();
                    return Ok(());
                }
            }
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

    // Create a String to store all output
    let mut output_content = String::new();

    output_content.push_str(&format!("Analyzing Rust files in directory: {}\n", target_dir.display()));
    if target_only {
        output_content.push_str("(Non-recursive analysis)\n");
    }

    // Print file-level summaries if requested
    if show_per_file {
        output_content.push_str("\nFile-Level Summary:\n");
        output_content.push_str("==================\n");
        for (path, summary) in &file_summaries {
            output_content.push_str(&format!("\n{}\n", path.display()));
            output_content.push_str(&format!("  Maximum Trait Depth: {}\n", summary.max_depth));
            output_content.push_str(&format!("  Trait Count: {}\n", summary.trait_count));
            output_content.push_str(&format!("  Implementation Count: {}\n", summary.impl_count));
        }
    }

    // Print directory-level summaries if requested
    if show_per_dir {
        output_content.push_str("\nDirectory-Level Summary (Recursive):\n");
        output_content.push_str("=================================\n");
        for (dir_path, analyzer) in &dir_summaries {
            let summary = analyzer.get_summary();
            output_content.push_str(&format!("\n{}\n", dir_path.display()));
            output_content.push_str(&format!("  Maximum Trait Depth: {}\n", summary.max_depth));
            output_content.push_str(&format!("  Trait Count: {}\n", summary.trait_count));
            output_content.push_str(&format!("  Implementation Count: {}\n", summary.impl_count));
        }
    }

    // Print target directory summary if requested
    if target_only {
        output_content.push_str("\nTarget Directory Summary:\n");
        output_content.push_str("=======================\n");
        if let Some(analyzer) = dir_summaries.get(&target_dir) {
            let summary = analyzer.get_summary();
            output_content.push_str(&format!("  Maximum Trait Depth: {}\n", summary.max_depth));
            output_content.push_str(&format!("  Trait Count: {}\n", summary.trait_count));
            output_content.push_str(&format!("  Implementation Count: {}\n", summary.impl_count));
        } else {
            output_content.push_str("No Rust files found in target directory\n");
        }
    }

    // Add global summary section
    let global_summary = trait_analyzer.get_summary();
    output_content.push_str("\nGlobal Summary:\n");
    output_content.push_str("==============\n");
    output_content.push_str(&format!("Overall Maximum Trait Depth: {}\n", global_summary.max_depth));
    output_content.push_str(&format!("Total Trait Count: {}\n", global_summary.trait_count));
    output_content.push_str(&format!("Total Implementation Count: {}\n", global_summary.impl_count));

    // Print final detailed analysis if no specific summary was requested
    if !show_per_file && !show_per_dir && !target_only {
        output_content.push_str("\nFinal Analysis Report:\n");
        output_content.push_str("====================\n");
        
        output_content.push_str("\nTrait Hierarchy:\n");
        for (trait_name, supertraits) in &trait_analyzer.trait_graph {
            output_content.push_str(&format!("{} -> {:?}\n", trait_name, supertraits));
        }

        output_content.push_str("\nType Implementations and Maximum Trait Depth:\n");
        for (type_name, traits) in &trait_analyzer.impl_map {
            output_content.push_str(&format!("\n{} implements:\n", type_name));
            for trait_name in traits {
                output_content.push_str(&format!("  - {}\n", trait_name));
            }
            let depth = trait_analyzer.calculate_max_depth(type_name);
            output_content.push_str(&format!("Maximum trait depth: {}\n", depth));
        }
    }

    // Write output to file or stdout
    write_output(output_content, output_file.as_deref())?;

    Ok(())
} 