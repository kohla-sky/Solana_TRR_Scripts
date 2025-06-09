use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::fs;
use syn::{parse_file, Item, Fields, Type};
use quote::quote;

/// Represents a struct's dependency information
#[derive(Debug)]
struct StructInfo {
    name: String,
    field_types: Vec<String>,
}

/// Calculates the maximum depth of nested struct compositions
fn calculate_max_struct_depth(
    struct_map: &HashMap<String, Vec<String>>,
    struct_name: &str,
    visited: &mut HashSet<String>,
    curr_depth: usize,
) -> usize {
    // Base case: if we've seen this struct before, return current depth to avoid cycles
    if !visited.insert(struct_name.to_string()) {
        return curr_depth;
    }

    let mut max_depth = curr_depth;

    // If the struct exists in our map, check its field types
    if let Some(field_types) = struct_map.get(struct_name) {
        for field_type in field_types {
            // Only recurse if the field type is in our struct map
            if struct_map.contains_key(field_type) {
                let depth = calculate_max_struct_depth(
                    struct_map,
                    field_type,
                    visited,
                    curr_depth + 1,
                );
                max_depth = max_depth.max(depth);
            }
        }
    }

    visited.remove(struct_name);
    max_depth
}

/// Extracts the type name from a syn::Type
fn extract_type_name(ty: &Type) -> String {
    let tokens = quote!(#ty);
    tokens.to_string().replace(' ', "")
}

/// Processes a single file and extracts struct information
fn process_file(path: &Path) -> std::io::Result<Vec<StructInfo>> {
    println!("Processing file: {:?}", path);
    let content = fs::read_to_string(path)?;
    println!("File content length: {}", content.len());
    match parse_file(&content) {
        Ok(file) => {
            let mut structs = Vec::new();

            for item in file.items {
                if let Item::Struct(item_struct) = item {
                    let struct_name = item_struct.ident.to_string();
                    println!("Found struct: {}", struct_name);
                    let mut field_types = Vec::new();

                    if let Fields::Named(fields) = item_struct.fields {
                        for field in fields.named {
                            let type_name = extract_type_name(&field.ty);
                            field_types.push(type_name);
                        }
                    }

                    structs.push(StructInfo {
                        name: struct_name,
                        field_types,
                    });
                }
            }

            println!("Found {} structs in file", structs.len());
            Ok(structs)
        }
        Err(e) => {
            eprintln!("Error parsing file {:?}: {}", path, e);
            Ok(Vec::new())
        }
    }
}

/// Recursively process directories and files
fn process_directory(path: &Path) -> std::io::Result<Vec<StructInfo>> {
    let mut structs = Vec::new();

    if path.is_file() {
        if path.extension().and_then(|s| s.to_str()) == Some("rs") {
            match process_file(path) {
                Ok(file_structs) => structs.extend(file_structs),
                Err(e) => eprintln!("Error processing file {:?}: {}", path, e),
            }
        }
    } else if path.is_dir() {
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let path = entry.path();
            structs.extend(process_directory(&path)?);
        }
    }

    Ok(structs)
}

/// Main function to analyze struct composition depth
fn analyze_struct_depth(source_path: &Path) -> std::io::Result<(usize, HashMap<String, Vec<String>>)> {
    let mut struct_map: HashMap<String, Vec<String>> = HashMap::new();
    let mut max_global_depth = 0;

    // Process all files recursively
    let structs = process_directory(source_path)?;
    
    // Build the struct map
    for struct_info in structs {
        struct_map.insert(struct_info.name, struct_info.field_types);
    }

    // Calculate maximum depth for each struct
    for struct_name in struct_map.keys() {
        let mut visited = HashSet::new();
        let depth = calculate_max_struct_depth(&struct_map, struct_name, &mut visited, 1);
        max_global_depth = max_global_depth.max(depth);
    }

    Ok((max_global_depth, struct_map))
}

fn print_help() {
    println!("Maximum Struct Composition Depth (MSCD) Analyzer");
    println!("\nUsage:");
    println!("  ./mscd-analyzer <directory>");
    println!("\nOptions:");
    println!("  -h, --help    Show this help message");
    println!("\nExample:");
    println!("  ./mscd-analyzer ./src");
    println!("  ./mscd-analyzer /path/to/rust/files");
}

fn main() -> std::io::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    
    if args.len() < 2 || args[1] == "-h" || args[1] == "--help" {
        print_help();
        return Ok(());
    }

    let source_path = PathBuf::from(&args[1]);

    if !source_path.exists() {
        eprintln!("Error: Directory '{}' does not exist", source_path.display());
        return Ok(());
    }

    match analyze_struct_depth(&source_path) {
        Ok((depth, struct_map)) => {
            println!("\nAnalysis Results:");
            println!("=================");
            println!("Maximum struct composition depth: {}", depth);
            println!("\nStruct count: {}", struct_map.len());
            
            if depth > 0 {
                println!("\nStructs with their field types:");
                println!("============================");
                for (struct_name, field_types) in struct_map {
                    println!("\n{}", struct_name);
                    for field_type in field_types {
                        println!("  - {}", field_type);
                    }
                }
            }
            
            Ok(())
        }
        Err(e) => {
            eprintln!("Error analyzing struct depth: {}", e);
            Err(e)
        }
    }
}
