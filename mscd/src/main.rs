use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::fs;
use std::process::Command;
use syn::{parse_file, Item, Fields, Type, GenericArgument, PathArguments, UseTree, ItemUse, ItemMod};
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

/// Represents an import/use statement
#[derive(Debug, Clone)]
struct ImportInfo {
    /// The imported path (e.g., "a::b::Inner")
    full_path: String,
    /// The local name it's imported as (e.g., "Inner" or "Alias")
    local_name: String,
    /// The module where this import exists
    module_path: Vec<String>,
}

/// Context for parsing with module information
#[derive(Debug)]
struct ParseContext {
    current_module_path: Vec<String>,
    structs: Vec<StructInfo>,
    type_aliases: Vec<TypeAlias>,
    imports: Vec<ImportInfo>,
    /// Maps module names to their file paths for out-of-line modules
    module_files: HashMap<String, PathBuf>,
    /// Root directory for resolving relative paths
    root_dir: PathBuf,
}

impl ParseContext {
    fn new() -> Self {
        Self {
            current_module_path: Vec::new(),
            structs: Vec::new(),
            type_aliases: Vec::new(),
            imports: Vec::new(),
            module_files: HashMap::new(),
            root_dir: PathBuf::new(),
        }
    }

    fn with_root_dir(root_dir: PathBuf) -> Self {
        Self {
            current_module_path: Vec::new(),
            structs: Vec::new(),
            type_aliases: Vec::new(),
            imports: Vec::new(),
            module_files: HashMap::new(),
            root_dir,
        }
    }

    fn with_module(&self, module_name: String) -> Self {
        let mut new_path = self.current_module_path.clone();
        new_path.push(module_name);
        Self {
            current_module_path: new_path,
            structs: self.structs.clone(),
            type_aliases: self.type_aliases.clone(),
            imports: self.imports.clone(),
            module_files: self.module_files.clone(),
            root_dir: self.root_dir.clone(),
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
fn extract_type_dependencies(ty: &Type, context: &ParseContext) -> Vec<String> {
    let mut dependencies = Vec::new();
    
    match ty {
        // Handle path types (most common case)
        Type::Path(type_path) => {
            dependencies.extend(extract_path_dependencies(&type_path.path, context));
        }
        // Handle references (&T)
        Type::Reference(type_ref) => {
            dependencies.extend(extract_type_dependencies(&type_ref.elem, context));
        }
        // Handle slices ([T])
        Type::Slice(type_slice) => {
            dependencies.extend(extract_type_dependencies(&type_slice.elem, context));
        }
        // Handle arrays ([T; N])
        Type::Array(type_array) => {
            dependencies.extend(extract_type_dependencies(&type_array.elem, context));
        }
        // Handle tuples - include ALL elements
        Type::Tuple(type_tuple) => {
            for elem in &type_tuple.elems {
                dependencies.extend(extract_type_dependencies(elem, context));
            }
        }
        // Handle raw pointers (*const T, *mut T)
        Type::Ptr(type_ptr) => {
            dependencies.extend(extract_type_dependencies(&type_ptr.elem, context));
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
fn extract_path_dependencies(path: &syn::Path, context: &ParseContext) -> Vec<String> {
    let mut dependencies = Vec::new();
    
    // Get the full path as a string
    let path_str = path.segments.iter()
        .map(|segment| segment.ident.to_string())
        .collect::<Vec<_>>()
        .join("::");
    
    // Handle Self keyword
    let resolved_path = if path_str == "Self" {
        // Replace Self with current struct name (we'll handle this in the calling context)
        path_str
    } else {
        // Resolve the path through imports and relative paths
        resolve_path(&path_str, context)
    };
    
    // Add the main type if it's not primitive
    if !is_primitive_type(&resolved_path) {
        dependencies.push(resolved_path);
    }
    
    // Extract generic arguments
    for segment in &path.segments {
        if let PathArguments::AngleBracketed(args) = &segment.arguments {
            for arg in &args.args {
                if let GenericArgument::Type(ty) = arg {
                    dependencies.extend(extract_type_dependencies(ty, context));
                }
            }
        }
    }
    
    dependencies
}

/// Resolve a path string through imports, aliases, and relative paths
fn resolve_path(path_str: &str, context: &ParseContext) -> String {
    // Handle relative paths
    let normalized_path = normalize_relative_path(path_str, &context.current_module_path);
    
    // Check if it's an import alias
    if let Some(import) = context.imports.iter().find(|imp| imp.local_name == normalized_path) {
        return import.full_path.clone();
    }
    
    // Check if it's a simple unqualified name that might be imported
    if !normalized_path.contains("::") {
        // Look for imports that end with this name
        if let Some(import) = context.imports.iter().find(|imp| {
            imp.full_path.split("::").last() == Some(&normalized_path)
        }) {
            return import.full_path.clone();
        }
    }
    
    normalized_path
}

/// Normalize relative paths (crate::, self::, super::)
fn normalize_relative_path(path_str: &str, current_module: &[String]) -> String {
    if path_str.starts_with("crate::") {
        // crate:: means from the root
        path_str.strip_prefix("crate::").unwrap().to_string()
    } else if path_str.starts_with("self::") {
        // self:: means current module
        let relative = path_str.strip_prefix("self::").unwrap();
        if current_module.is_empty() {
            relative.to_string()
        } else {
            format!("{}::{}", current_module.join("::"), relative)
        }
    } else if path_str.starts_with("super::") {
        // super:: means parent module
        let relative = path_str.strip_prefix("super::").unwrap();
        if current_module.len() <= 1 {
            relative.to_string()
        } else {
            let parent_path = &current_module[..current_module.len() - 1];
            format!("{}::{}", parent_path.join("::"), relative)
        }
    } else if current_module.is_empty() || path_str.contains("::") {
        // Already absolute or we're at root
        path_str.to_string()
    } else {
        // Relative to current module
        format!("{}::{}", current_module.join("::"), path_str)
    }
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
    // First pass: collect imports and module declarations
    for item in items {
        match item {
            Item::Use(item_use) => {
                process_use_item(item_use, context);
            }
            Item::Mod(item_mod) => {
                if item_mod.content.is_none() {
                    // Out-of-line module (mod x;)
                    let module_name = item_mod.ident.to_string();
                    let module_path = resolve_module_file(&module_name, context);
                    if let Some(path) = module_path {
                        context.module_files.insert(module_name, path);
                    }
                }
            }
            _ => {}
        }
    }
    
    // Second pass: process structs and other items
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
                            let mut deps = extract_type_dependencies(&field.ty, context);
                            // Handle Self references
                            deps = deps.into_iter().map(|dep| {
                                if dep == "Self" {
                                    struct_name.clone()
                                } else {
                                    dep
                                }
                            }).collect();
                            field_types.extend(deps);
                        }
                    }
                    // Tuple structs (unnamed fields)
                    Fields::Unnamed(fields) => {
                        for field in &fields.unnamed {
                            let mut deps = extract_type_dependencies(&field.ty, context);
                            // Handle Self references
                            deps = deps.into_iter().map(|dep| {
                                if dep == "Self" {
                                    struct_name.clone()
                                } else {
                                    dep
                                }
                            }).collect();
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
                    context.imports.extend(nested_context.imports);
                } else {
                    // Out-of-line module - process the file if we found it
                    let module_name = item_mod.ident.to_string();
                    if let Some(module_file) = context.module_files.get(&module_name).cloned() {
                        if let Ok(nested_context) = process_file(&module_file) {
                            let mut nested_context_with_module = nested_context;
                            nested_context_with_module.current_module_path = 
                                [context.current_module_path.clone(), vec![module_name]].concat();
                            
                            context.structs.extend(nested_context_with_module.structs);
                            context.type_aliases.extend(nested_context_with_module.type_aliases);
                            context.imports.extend(nested_context_with_module.imports);
                        }
                    }
                }
            }
            Item::Type(item_type) => {
                // Handle type aliases
                let alias_name = item_type.ident.to_string();
                let target_deps = extract_type_dependencies(&item_type.ty, context);
                
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

/// Process a use statement to extract import information
fn process_use_item(item_use: &ItemUse, context: &mut ParseContext) {
    process_use_tree(&item_use.tree, Vec::new(), context);
}

/// Recursively process use tree to extract all imports
fn process_use_tree(tree: &UseTree, prefix: Vec<String>, context: &mut ParseContext) {
    match tree {
        UseTree::Path(use_path) => {
            let mut new_prefix = prefix;
            new_prefix.push(use_path.ident.to_string());
            process_use_tree(&use_path.tree, new_prefix, context);
        }
        UseTree::Name(use_name) => {
            let mut full_path = prefix;
            full_path.push(use_name.ident.to_string());
            let full_path_str = full_path.join("::");
            let local_name = use_name.ident.to_string();
            
            context.imports.push(ImportInfo {
                full_path: full_path_str,
                local_name,
                module_path: context.current_module_path.clone(),
            });
        }
        UseTree::Rename(use_rename) => {
            let mut full_path = prefix;
            full_path.push(use_rename.ident.to_string());
            let full_path_str = full_path.join("::");
            let local_name = use_rename.rename.to_string();
            
            context.imports.push(ImportInfo {
                full_path: full_path_str,
                local_name,
                module_path: context.current_module_path.clone(),
            });
        }
        UseTree::Glob(_) => {
            // For glob imports, we'd need more sophisticated handling
            // For now, we'll skip them as they're complex to resolve
        }
        UseTree::Group(use_group) => {
            for tree in &use_group.items {
                process_use_tree(tree, prefix.clone(), context);
            }
        }
    }
}

/// Resolve the file path for an out-of-line module
fn resolve_module_file(module_name: &str, context: &ParseContext) -> Option<PathBuf> {
    let base_path = if context.current_module_path.is_empty() {
        context.root_dir.clone()
    } else {
        context.root_dir.join(context.current_module_path.join("/"))
    };
    
    // Try module_name.rs first
    let rs_path = base_path.join(format!("{}.rs", module_name));
    if rs_path.exists() {
        return Some(rs_path);
    }
    
    // Try module_name/mod.rs
    let mod_path = base_path.join(module_name).join("mod.rs");
    if mod_path.exists() {
        return Some(mod_path);
    }
    
    None
}

/// Processes a single file and extracts struct information
fn process_file(path: &Path) -> std::io::Result<ParseContext> {
    println!("Processing file: {:?}", path);
    let content = fs::read_to_string(path)?;
    println!("File content length: {}", content.len());
    
    match parse_file(&content) {
        Ok(file) => {
            let root_dir = path.parent().unwrap_or(Path::new(".")).to_path_buf();
            let mut context = ParseContext::with_root_dir(root_dir);
            process_items(&file.items, &mut context);
            
            println!("Found {} structs, {} type aliases, and {} imports in file", 
                     context.structs.len(), context.type_aliases.len(), context.imports.len());
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
    let root_dir = if path.is_file() {
        path.parent().unwrap_or(Path::new(".")).to_path_buf()
    } else {
        path.to_path_buf()
    };
    
    let mut combined_context = ParseContext::with_root_dir(root_dir);

    if path.is_file() {
        if path.extension().and_then(|s| s.to_str()) == Some("rs") {
            match process_file(path) {
                Ok(file_context) => {
                    combined_context.structs.extend(file_context.structs);
                    combined_context.type_aliases.extend(file_context.type_aliases);
                    combined_context.imports.extend(file_context.imports);
                }
                Err(e) => eprintln!("Error processing file {:?}: {}", path, e),
            }
        }
    } else if path.is_dir() {
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let entry_path = entry.path();
            let sub_context = process_directory(&entry_path)?;
            combined_context.structs.extend(sub_context.structs);
            combined_context.type_aliases.extend(sub_context.type_aliases);
            combined_context.imports.extend(sub_context.imports);
        }
    }

    Ok(combined_context)
}

/// Resolve type aliases to their final types, handling chains and multi-target aliases
fn resolve_type_aliases(
    field_types: &[String], 
    type_aliases: &HashMap<String, String>,
    struct_names: &HashSet<String>,
    current_module_path: &[String]
) -> Vec<String> {
    field_types.iter().flat_map(|field_type| {
        // Resolve alias chains
        let mut resolved_types = resolve_alias_chain(field_type, type_aliases);
        
        // If no aliases were resolved, use the original type
        if resolved_types.is_empty() {
            resolved_types.push(field_type.clone());
        }
        
        // For each resolved type, try to resolve relative module paths
        resolved_types.into_iter().map(|resolved_type| {
            if !resolved_type.contains("::") && !current_module_path.is_empty() {
                let full_path = format!("{}::{}", current_module_path.join("::"), resolved_type);
                if struct_names.contains(&full_path) {
                    return full_path;
                }
            }
            resolved_type
        }).collect::<Vec<_>>()
    }).collect()
}

/// Resolve a single type through alias chains, handling multi-target aliases
fn resolve_alias_chain(type_name: &str, type_aliases: &HashMap<String, String>) -> Vec<String> {
    let mut result = Vec::new();
    let mut current = type_name.to_string();
    let mut visited = HashSet::new();
    
    // Handle potential generic types like M<K, V>
    if current.contains('<') {
        // Extract the base type and generic arguments
        if let Some(base_end) = current.find('<') {
            let base_type = &current[..base_end];
            let generics_part = &current[base_end..];
            
            // Try to resolve the base type
            if let Some(target) = type_aliases.get(base_type) {
                // If the target also has generics, we need to substitute
                if target.contains('<') {
                    result.push(current); // Keep original for now
                } else {
                    result.push(format!("{}{}", target, generics_part));
                }
            } else {
                result.push(current);
            }
        } else {
            result.push(current);
        }
    } else {
        // Simple alias chain resolution
        while let Some(target) = type_aliases.get(&current) {
            if !visited.insert(current.clone()) {
                // Circular alias, break
                break;
            }
            current = target.clone();
        }
        result.push(current);
    }
    
    result
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
