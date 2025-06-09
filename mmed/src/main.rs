use syn::{visit::Visit, Attribute, Meta};
use syn::__private::ToTokens;
use proc_macro2::{TokenStream, TokenTree};
use std::{fs, path::PathBuf, collections::HashMap, collections::HashSet};
use clap::Parser;
use walkdir::WalkDir;

#[derive(Parser, Debug)]
#[clap(author, version, about)]
struct Args {
    /// Path to the directory containing Rust files to analyze
    #[clap(short, long)]
    dir: PathBuf,
}

#[derive(Debug, Hash, Eq, PartialEq, Clone)]
enum WarningType {
    ProcMacro(String),
    CompilerHelper(String),
    MacroRepetition(String),
    StringLiteralMacro,
}

struct MacroDepthVisitor {
    current_depth: usize,
    max_depth: usize,
    current_macro: Option<String>,
    known_proc_macros: HashSet<String>,
    warnings: Vec<(WarningType, String)>,
}

impl MacroDepthVisitor {
    fn new() -> Self {
        let mut known_proc_macros = HashSet::new();
        // Common proc macros that typically generate deep macro trees
        known_proc_macros.insert("derive".to_string());
        known_proc_macros.insert("proc_macro".to_string());
        known_proc_macros.insert("proc_macro_derive".to_string());
        known_proc_macros.insert("anchor_lang".to_string());
        known_proc_macros.insert("serde".to_string());

        MacroDepthVisitor {
            current_depth: 0,
            max_depth: 0,
            current_macro: None,
            known_proc_macros,
            warnings: Vec::new(),
        }
    }

    fn scan_token_stream(&mut self, tokens: &TokenStream) {
        let mut iter = tokens.clone().into_iter().peekable();
        
        while let Some(token) = iter.next() {
            match token {
                TokenTree::Ident(ident) => {
                    let ident_str = ident.to_string();
                    
                    // Check for macro pattern: Ident + '!' + Group
                    if let Some(TokenTree::Punct(punct)) = iter.peek() {
                        if punct.as_char() == '!' {
                            iter.next(); // consume '!'
                            
                            // Special handling for known compiler-generated macros
                            if ["format_args", "assert", "debug_assert", "print", "println", "write", "writeln"]
                                .contains(&ident_str.as_str()) {
                                self.warnings.push((
                                    WarningType::CompilerHelper(ident_str.clone()),
                                    format!("Note: Found compiler helper macro '{}!' - depth might be affected", ident_str)
                                ));
                            }

                            self.current_macro = Some(ident_str);
                            self.current_depth += 1;
                            self.max_depth = self.max_depth.max(self.current_depth);

                            // Process the macro body if it exists
                            if let Some(TokenTree::Group(group)) = iter.next() {
                                // Check for repetition patterns
                                let stream_str = group.stream().to_string();
                                if stream_str.contains("$(") && stream_str.contains(")*") {
                                    let macro_name = self.current_macro.as_ref().unwrap_or(&"unknown".to_string()).clone();
                                    self.warnings.push((
                                        WarningType::MacroRepetition(macro_name.clone()),
                                        format!("Warning: Macro '{}!' contains repetition pattern - actual depth may be higher", macro_name)
                                    ));
                                }
                                
                                self.scan_token_stream(&group.stream());
                            }

                            self.current_depth = self.current_depth.saturating_sub(1);
                        }
                    }
                }
                TokenTree::Group(group) => {
                    self.scan_token_stream(&group.stream());
                }
                TokenTree::Literal(lit) => {
                    // Scan string literals for potential macro calls
                    let lit_str = lit.to_string();
                    if lit_str.contains("!") {
                        self.warnings.push((
                            WarningType::StringLiteralMacro,
                            "Note: Found '!' in string literal - might be a hidden macro call".to_string()
                        ));
                    }
                }
                _ => {}
            }
        }
    }

    fn scan_attribute(&mut self, attr: &Attribute) {
        // Special handling for proc-macro attributes
        if let Ok(meta) = attr.parse_args::<Meta>() {
            if let Meta::List(list) = meta {
                let path_str = list.path.to_token_stream().to_string();
                
                if self.known_proc_macros.contains(&path_str) {
                    self.warnings.push((
                        WarningType::ProcMacro(path_str.clone()),
                        format!("Warning: Found proc-macro attribute '{}' - actual macro depth may be significantly higher", path_str)
                    ));
                    // Assume proc-macros typically generate at least 3 levels of macro calls
                    self.max_depth = self.max_depth.max(3);
                }
            }
        }
        
        if let Ok(tokens) = attr.parse_args::<TokenStream>() {
            self.scan_token_stream(&tokens);
        }
    }
}

impl<'ast> Visit<'ast> for MacroDepthVisitor {
    fn visit_macro(&mut self, mac: &'ast syn::Macro) {
        if let Some(ident) = mac.path.segments.last() {
            self.current_macro = Some(ident.ident.to_string());
        }
        
        self.current_depth += 1;
        self.max_depth = self.max_depth.max(self.current_depth);
        self.scan_token_stream(&mac.tokens);
        self.current_depth = self.current_depth.saturating_sub(1);
    }

    fn visit_attribute(&mut self, attr: &'ast Attribute) {
        self.scan_attribute(attr);
        syn::visit::visit_attribute(self, attr);
    }
}

fn analyze_file(path: &PathBuf) -> Result<(usize, Vec<(WarningType, String)>), Box<dyn std::error::Error>> {
    let source = fs::read_to_string(path)?;
    let syntax = syn::parse_file(&source)?;
    
    let mut visitor = MacroDepthVisitor::new();
    visitor.visit_file(&syntax);
    
    println!("File: {}", path.display());
    println!("Maximum macro nesting depth: {}", visitor.max_depth);
    
    if !visitor.warnings.is_empty() {
        println!("\nAnalysis warnings:");
        for (_, warning) in &visitor.warnings {
            println!("- {}", warning);
        }
        println!();
    }
    
    Ok((visitor.max_depth, visitor.warnings))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    
    let mut max_overall_depth = 0;
    let mut files_analyzed = 0;
    let mut all_warnings: Vec<(WarningType, String)> = Vec::new();
    
    // Walk through all files in the directory
    for entry in WalkDir::new(&args.dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map_or(false, |ext| ext == "rs"))
    {
        match analyze_file(&entry.path().to_path_buf()) {
            Ok((depth, warnings)) => {
                max_overall_depth = max_overall_depth.max(depth);
                files_analyzed += 1;
                all_warnings.extend(warnings);
            }
            Err(e) => {
                eprintln!("Error analyzing {}: {}", entry.path().display(), e);
            }
        }
    }
    
    println!("\nAnalysis Summary:");
    println!("Files analyzed: {}", files_analyzed);
    println!("Maximum macro nesting depth across all files: {}", max_overall_depth);
    
    if !all_warnings.is_empty() {
        let mut warning_counts: HashMap<WarningType, usize> = HashMap::new();
        for (warning_type, _) in &all_warnings {
            *warning_counts.entry(warning_type.clone()).or_insert(0) += 1;
        }

        println!("\nWarning Statistics:");
        for (warning_type, count) in warning_counts {
            match warning_type {
                WarningType::ProcMacro(name) => {
                    println!("Procedural macro '{}': {} instances", name, count);
                }
                WarningType::CompilerHelper(name) => {
                    println!("Compiler helper macro '{}': {} instances", name, count);
                }
                WarningType::MacroRepetition(name) => {
                    println!("Macro with repetition pattern '{}': {} instances", name, count);
                }
                WarningType::StringLiteralMacro => {
                    println!("Potential macro calls in string literals: {} instances", count);
                }
            }
        }

        println!("\nDetailed Warnings:");
        let mut unique_warnings: HashSet<_> = all_warnings.into_iter().map(|(_, msg)| msg).collect();
        for warning in unique_warnings {
            println!("- {}", warning);
        }
    }
    
    Ok(())
}

