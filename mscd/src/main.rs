use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::fs;
use std::process::Command;
use syn::{parse_file, Item, Fields, Type, GenericArgument, PathArguments};
use quote::quote;
use tempfile::TempDir;
use url::Url;

/// Represents a struct's dependency information
#[derive(Debug, Clone)]
struct StructInfo {
    name: String,
    field_types: Vec<String>,
    module_path: Vec<String>, // Track the module path for this struct
}

/// Represents a type alias
#[derive(Debug, Clone)]
struct TypeAlias {
    name: String,
    target_type: String,
    module_path: Vec<String>,
}

/// Context for parsing with module information
#[derive(Debug)]
struct ParseContext {
    current_module_path: Vec<String>,
    structs: Vec<StructInfo>,
    type_aliases: Vec<TypeAlias>,
}

impl ParseContext {
    fn new() -> Self {
        Self {
            current_module_path: Vec::new(),
            structs: Vec::new(),
            type_aliases: Vec::new(),
        }
    }

    fn with_module(&self, module_name: String) -> Self {
        let mut new_path = self.current_module_path.clone();
        new_path.push(module_name);
        Self {
            current_module_path: new_path,
            structs: self.structs.clone(),
            type_aliases: self.type_aliases.clone(),
        }
    }
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

/// Extracts all type dependencies from a syn::Type, handling wrappers and complex types
fn extract_type_dependencies(ty: &Type) -> Vec<String> {
    let mut dependencies = Vec::new();
    
    match ty {
        // Handle path types (most common case)
        Type::Path(type_path) => {
            dependencies.extend(extract_path_dependencies(&type_path.path));
        }
        // Handle references (&T)
        Type::Reference(type_ref) => {
            dependencies.extend(extract_type_dependencies(&type_ref.elem));
        }
        // Handle slices ([T])
        Type::Slice(type_slice) => {
            dependencies.extend(extract_type_dependencies(&type_slice.elem));
        }
        // Handle arrays ([T; N])
        Type::Array(type_array) => {
            dependencies.extend(extract_type_dependencies(&type_array.elem));
        }
        // Handle tuples
        Type::Tuple(type_tuple) => {
            for elem in &type_tuple.elems {
                dependencies.extend(extract_type_dependencies(elem));
            }
        }
        // Handle function pointers and other types
        _ => {
            // For other types, convert to string and try to extract
            let tokens = quote!(#ty);
            let type_str = tokens.to_string().replace(' ', "");
            if !type_str.is_empty() && !is_primitive_type(&type_str) {
                dependencies.push(type_str);
            }
        }
    }
    
    dependencies
}

/// Extract dependencies from a syn::Path, handling generics and module paths
fn extract_path_dependencies(path: &syn::Path) -> Vec<String> {
    let mut dependencies = Vec::new();
    
    // Get the full path as a string
    let full_path = path.segments.iter()
        .map(|segment| segment.ident.to_string())
        .collect::<Vec<_>>()
        .join("::");
    
    // Add the main type if it's not primitive
    if !is_primitive_type(&full_path) {
        dependencies.push(full_path);
    }
    
    // Extract generic arguments
    for segment in &path.segments {
        if let PathArguments::AngleBracketed(args) = &segment.arguments {
            for arg in &args.args {
                if let GenericArgument::Type(ty) = arg {
                    dependencies.extend(extract_type_dependencies(ty));
                }
            }
        }
    }
    
    dependencies
}

/// Check if a type is a primitive type
fn is_primitive_type(type_name: &str) -> bool {
    matches!(type_name, 
        "u8" | "u16" | "u32" | "u64" | "u128" | "usize" |
        "i8" | "i16" | "i32" | "i64" | "i128" | "isize" |
        "f32" | "f64" | "bool" | "char" | "str" | "()" |
        "String" | "Vec" | "Option" | "Result" | "Box" | "Rc" | "Arc" |
        "HashMap" | "HashSet" | "BTreeMap" | "BTreeSet"
    )
}

/// Process items within a module or file, handling nested structures
fn process_items(items: &[Item], context: &mut ParseContext) {
    for item in items {
        match item {
            Item::Struct(item_struct) => {
                let struct_name = item_struct.ident.to_string();
                println!("Found struct: {} in module: {:?}", struct_name, context.current_module_path);
                let mut field_types = Vec::new();

                match &item_struct.fields {
                    // Named fields
                    Fields::Named(fields) => {
                        for field in &fields.named {
                            let deps = extract_type_dependencies(&field.ty);
                            field_types.extend(deps);
                        }
                    }
                    // Tuple structs (unnamed fields)
                    Fields::Unnamed(fields) => {
                        for field in &fields.unnamed {
                            let deps = extract_type_dependencies(&field.ty);
                            field_types.extend(deps);
                        }
                    }
                    // Unit structs (no fields)
                    Fields::Unit => {}
                }

                // Create full struct name with module path
                let full_name = if context.current_module_path.is_empty() {
                    struct_name.clone()
                } else {
                    format!("{}::{}", context.current_module_path.join("::"), struct_name)
                };

                context.structs.push(StructInfo {
                    name: full_name,
                    field_types,
                    module_path: context.current_module_path.clone(),
                });
            }
            Item::Mod(item_mod) => {
                if let Some((_, items)) = &item_mod.content {
                    // Process inline module
                    let module_name = item_mod.ident.to_string();
                    let mut nested_context = context.with_module(module_name);
                    process_items(items, &mut nested_context);
                    
                    // Merge results back
                    context.structs.extend(nested_context.structs);
                    context.type_aliases.extend(nested_context.type_aliases);
                }
            }
            Item::Type(item_type) => {
                // Handle type aliases
                let alias_name = item_type.ident.to_string();
                let target_deps = extract_type_dependencies(&item_type.ty);
                
                if let Some(target_type) = target_deps.first() {
                    let full_alias_name = if context.current_module_path.is_empty() {
                        alias_name.clone()
                    } else {
                        format!("{}::{}", context.current_module_path.join("::"), alias_name)
                    };
                    
                    context.type_aliases.push(TypeAlias {
                        name: full_alias_name,
                        target_type: target_type.clone(),
                        module_path: context.current_module_path.clone(),
                    });
                }
            }
            _ => {}
        }
    }
}

/// Processes a single file and extracts struct information
fn process_file(path: &Path) -> std::io::Result<ParseContext> {
    println!("Processing file: {:?}", path);
    let content = fs::read_to_string(path)?;
    println!("File content length: {}", content.len());
    
    match parse_file(&content) {
        Ok(file) => {
            let mut context = ParseContext::new();
            process_items(&file.items, &mut context);
            
            println!("Found {} structs and {} type aliases in file", 
                     context.structs.len(), context.type_aliases.len());
            Ok(context)
        }
        Err(e) => {
            eprintln!("Error parsing file {:?}: {}", path, e);
            Ok(ParseContext::new())
        }
    }
}

/// Recursively process directories and files
fn process_directory(path: &Path) -> std::io::Result<ParseContext> {
    let mut combined_context = ParseContext::new();

    if path.is_file() {
        if path.extension().and_then(|s| s.to_str()) == Some("rs") {
            match process_file(path) {
                Ok(file_context) => {
                    combined_context.structs.extend(file_context.structs);
                    combined_context.type_aliases.extend(file_context.type_aliases);
                }
                Err(e) => eprintln!("Error processing file {:?}: {}", path, e),
            }
        }
    } else if path.is_dir() {
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let path = entry.path();
            let sub_context = process_directory(&path)?;
            combined_context.structs.extend(sub_context.structs);
            combined_context.type_aliases.extend(sub_context.type_aliases);
        }
    }

    Ok(combined_context)
}

/// Resolve type aliases to their final types and normalize module paths
fn resolve_type_aliases(
    field_types: &[String], 
    type_aliases: &HashMap<String, String>,
    struct_names: &HashSet<String>,
    current_module_path: &[String]
) -> Vec<String> {
    field_types.iter().map(|field_type| {
        // First try to resolve through aliases
        let mut resolved_type = field_type.clone();
        let mut visited = HashSet::new();
        
        while let Some(target) = type_aliases.get(&resolved_type) {
            if !visited.insert(resolved_type.clone()) {
                // Circular alias, break
                break;
            }
            resolved_type = target.clone();
        }
        
        // Then try to resolve relative module paths to absolute paths
        // If the type doesn't contain "::" and we have a current module context,
        // try to find it in the current module first
        if !resolved_type.contains("::") && !current_module_path.is_empty() {
            let full_path = format!("{}::{}", current_module_path.join("::"), resolved_type);
            if struct_names.contains(&full_path) {
                return full_path;
            }
        }
        
        resolved_type
    }).collect()
}

/// Main function to analyze struct composition depth
fn analyze_struct_depth(source_path: &Path) -> std::io::Result<(usize, HashMap<String, Vec<String>>)> {
    let mut struct_map: HashMap<String, Vec<String>> = HashMap::new();
    let mut type_alias_map: HashMap<String, String> = HashMap::new();
    let mut max_global_depth = 0;

    // Process all files recursively
    let context = process_directory(source_path)?;
    
    // Build the type alias map
    for type_alias in &context.type_aliases {
        type_alias_map.insert(type_alias.name.clone(), type_alias.target_type.clone());
    }
    
    // Collect all struct names for path resolution
    let struct_names: HashSet<String> = context.structs.iter()
        .map(|s| s.name.clone())
        .collect();
    
    // Build the struct map with resolved types
    for struct_info in &context.structs {
        let resolved_types = resolve_type_aliases(
            &struct_info.field_types, 
            &type_alias_map,
            &struct_names,
            &struct_info.module_path
        );
        struct_map.insert(struct_info.name.clone(), resolved_types);
    }

    // Calculate maximum depth for each struct
    for struct_name in struct_map.keys() {
        let mut visited = HashSet::new();
        let depth = calculate_max_struct_depth(&struct_map, struct_name, &mut visited, 1);
        max_global_depth = max_global_depth.max(depth);
    }

    Ok((max_global_depth, struct_map))
}

/// Clone a Git repository to a temporary directory using system git command
fn clone_repository(repo_url: &str) -> Result<TempDir, Box<dyn std::error::Error>> {
    println!("Cloning repository: {}", repo_url);
    
    let temp_dir = TempDir::new()?;
    let repo_path = temp_dir.path();
    
    let output = Command::new("git")
        .args(&["clone", repo_url, repo_path.to_str().unwrap()])
        .output()?;
    
    if !output.status.success() {
        let error_msg = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Git clone failed: {}", error_msg).into());
    }
    
    println!("Repository cloned to temporary directory");
    Ok(temp_dir)
}

/// Check if a string is a valid URL
fn is_url(s: &str) -> bool {
    Url::parse(s).is_ok()
}

fn print_help() {
    println!("Maximum Struct Composition Depth (MSCD) Analyzer");
    println!("\nUsage:");
    println!("  ./mscd-analyzer <directory>");
    println!("  ./mscd-analyzer --repo <repo_url_or_path> <relative_directory>");
    println!("\nOptions:");
    println!("  -h, --help                    Show this help message");
    println!("  --repo <repo_url_or_path>     Specify Git repository URL or local path");
    println!("\nExamples:");
    println!("  ./mscd-analyzer ./src");
    println!("  ./mscd-analyzer /path/to/rust/files");
    println!("  ./mscd-analyzer --repo https://github.com/user/repo.git src/");
    println!("  ./mscd-analyzer --repo git@github.com:user/repo.git ./lib");
    println!("  ./mscd-analyzer --repo /local/path/to/repo ./sample/src");
}

fn main() -> std::io::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    
    // Handle help
    if args.len() < 2 || args.contains(&"-h".to_string()) || args.contains(&"--help".to_string()) {
        print_help();
        return Ok(());
    }

    let (source_path, _temp_dir) = if args.len() >= 4 && args[1] == "--repo" {
        // Handle --repo flag: --repo <repo_url_or_path> <relative_path>
        let repo_input = &args[2];
        let relative_path = &args[3];
        
        if is_url(repo_input) || repo_input.starts_with("git@") {
            // Handle Git URL
            match clone_repository(repo_input) {
                Ok(temp_dir) => {
                    let repo_path = temp_dir.path();
                    let full_path = repo_path.join(relative_path);
                    
                    if !full_path.exists() {
                        eprintln!("Error: Path '{}' does not exist in cloned repository", relative_path);
                        return Ok(());
                    }
                    
                    println!("Analyzing: {}", relative_path);
                    (full_path, Some(temp_dir))
                }
                Err(e) => {
                    eprintln!("Error cloning repository '{}': {}", repo_input, e);
                    return Ok(());
                }
            }
        } else {
            // Handle local path
            let repo_path = PathBuf::from(repo_input);
            
            if !repo_path.exists() {
                eprintln!("Error: Repository path '{}' does not exist", repo_path.display());
                return Ok(());
            }
            
            if !repo_path.is_dir() {
                eprintln!("Error: Repository path '{}' is not a directory", repo_path.display());
                return Ok(());
            }
            
            let full_path = repo_path.join(relative_path);
            
            if !full_path.exists() {
                eprintln!("Error: Path '{}' does not exist in repository '{}'", 
                         relative_path, repo_path.display());
                return Ok(());
            }
            
            println!("Repository: {}", repo_path.display());
            println!("Analyzing: {}", relative_path);
            (full_path, None)
        }
    } else if args.len() >= 2 {
        // Handle direct path
        let path = PathBuf::from(&args[1]);
        
        if !path.exists() {
            eprintln!("Error: Directory '{}' does not exist", path.display());
            return Ok(());
        }
        
        (path, None)
    } else {
        print_help();
        return Ok(());
    };

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
