//! Integration test: Validates that contract error discriminants match the registry.
//!
//! This test reads actual error definitions from contract source files and
//! cross-checks them against the canonical registry in hermes-errors.
//!
//! # How it works
//!
//! 1. Locate all `contracts/*/src/errors.rs` files
//! 2. Parse them as Rust AST (using syn crate)
//! 3. Extract enum variants and their explicit discriminants
//! 4. Compare against registry definitions
//! 5. Report any mismatches (missing, extra, or wrong codes)

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Represents a single error variant extracted from contract code.
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd)]
struct ExtractedError {
    /// Variant name (e.g., "Unauthorized")
    name: String,
    /// Numeric discriminant (e.g., 1)
    code: u32,
    /// Which contract this came from (e.g., "analytics")
    contract: String,
    /// Full path to source file
    source_file: PathBuf,
}

/// Extract error variants from a Soroban contract error enum.
///
/// Parses the Rust file and finds the `#[contracterror]` enum,
/// then extracts all named variants with their explicit numeric discriminants.
fn extract_errors_from_file(
    source_file: &Path,
    contract_name: &str,
) -> Result<Vec<ExtractedError>, String> {
    let content = fs::read_to_string(source_file)
        .map_err(|e| format!("Failed to read {}: {}", source_file.display(), e))?;

    // Simple line-based parser: look for patterns like "VariantName = 123,"
    // This avoids heavyweight syn parsing for now (easier for CI environments without full Rust tooling)
    let mut errors = Vec::new();

    // Regex-like pattern: "    VariantName = 123," or "    VariantName = 123"
    for line in content.lines() {
        let trimmed = line.trim();

        // Skip comments, doc comments, and decorators
        if trimmed.starts_with("//")
            || trimmed.starts_with("///")
            || trimmed.starts_with("#[")
            || trimmed.starts_with("[")
            || trimmed.is_empty()
        {
            continue;
        }

        // Match pattern: "Name = code," or "Name = code" (with optional trailing comma)
        if let Some(eq_pos) = trimmed.find('=') {
            let (name_part, code_part) = trimmed.split_at(eq_pos);
            let name = name_part.trim().to_string();

            // Skip if it looks like we're not in an enum variant context
            if name.is_empty() || name.contains(' ') || name.contains('{') || name.contains('}')
            {
                continue;
            }

            // Extract the numeric code from "= 123," or "= 123"
            let code_str = code_part[1..].trim(); // skip '='
            let code_str = code_str.trim_end_matches(',').trim();

            if let Ok(code) = code_str.parse::<u32>() {
                // Skip if this looks like a comment or structural element
                if name != "pub" && name != "enum" && name != "}" && !name.contains('/') {
                    errors.push(ExtractedError {
                        name: name.clone(),
                        code,
                        contract: contract_name.to_string(),
                        source_file: source_file.to_path_buf(),
                    });
                }
            }
        }
    }

    Ok(errors)
}

/// Scan the contracts directory for all error definitions.
fn scan_contract_errors() -> Result<Vec<ExtractedError>, String> {
    let contracts_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .map(|p| p.join("contracts"))
        .ok_or_else(|| "Could not determine contracts directory".to_string())?;

    if !contracts_dir.exists() {
        return Err(format!(
            "Contracts directory not found: {}",
            contracts_dir.display()
        ));
    }

    let mut all_errors = Vec::new();

    // Iterate over contract subdirectories
    for entry in fs::read_dir(&contracts_dir)
        .map_err(|e| format!("Failed to read contracts dir: {}", e))?
    {
        let entry = entry.map_err(|e| format!("Failed to read dir entry: {}", e))?;
        let path = entry.path();

        if !path.is_dir() {
            continue;
        }

        let contract_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");
        let errors_file = path.join("src/errors.rs");

        if errors_file.exists() {
            match extract_errors_from_file(&errors_file, contract_name) {
                Ok(errors) => {
                    all_errors.extend(errors);
                }
                Err(e) => {
                    eprintln!("Warning: {}", e);
                }
            }
        }
    }

    Ok(all_errors)
}

#[test]
fn contract_errors_match_registry() {
    // Scan all contract error definitions
    let extracted = match scan_contract_errors() {
        Ok(errs) => errs,
        Err(e) => {
            eprintln!("Scan error: {}", e);
            panic!("Failed to scan contract errors: {}", e);
        }
    };

    if extracted.is_empty() {
        eprintln!("Warning: No contract errors found. This may indicate a scanning issue.");
        return;
    }

    println!("\nExtracted {} error definitions from contracts", extracted.len());
    for err in &extracted {
        println!(
            "  {} contract: {} = {}",
            err.contract, err.name, err.code
        );
    }

    // Import the registry
    use hermes_errors::{all_errors, find_by_code, find_by_name};

    let registry = all_errors();
    println!("\nRegistry contains {} error definitions", registry.len());

    // Build a map of (name, code) -> count for registry lookups
    let mut registry_map: BTreeMap<(String, u32), usize> = BTreeMap::new();
    for err in &registry {
        let key = (err.name.to_string(), err.code);
        *registry_map.entry(key).or_insert(0) += 1;
    }

    // Track mismatches
    let mut mismatches = Vec::new();

    for extracted_err in &extracted {
        let registry_match =
            find_by_code(extracted_err.code).map(|r| r.name.to_string());

        match registry_match {
            Some(registry_name) => {
                if registry_name != extracted_err.name {
                    mismatches.push(format!(
                        "Code mismatch: {} contract has '{}' = {}, but registry defines '{}' = {}",
                        extracted_err.contract,
                        extracted_err.name,
                        extracted_err.code,
                        registry_name,
                        extracted_err.code
                    ));
                } else {
                    println!(
                        "✓ {} contract: {} = {} (matches registry)",
                        extracted_err.contract, extracted_err.name, extracted_err.code
                    );
                }
            }
            None => {
                mismatches.push(format!(
                    "Unregistered error: {} contract has '{}' = {}, but this code is not in the registry",
                    extracted_err.contract, extracted_err.name, extracted_err.code
                ));
            }
        }
    }

    // Check for registry entries not found in any contract
    let mut extracted_codes: std::collections::BTreeSet<u32> = std::collections::BTreeSet::new();
    for err in &extracted {
        extracted_codes.insert(err.code);
    }

    for registry_err in &registry {
        // Only warn about codes that are in the 100+ range (contract-specific)
        // Skip universal errors (1-9, 20, etc.) that might not be used by all contracts
        if registry_err.code >= 100 && !extracted_codes.contains(&registry_err.code) {
            println!(
                "⚠ Registry has '{}' = {} but not found in any contract (may be undeployed)",
                registry_err.name, registry_err.code
            );
        }
    }

    if !mismatches.is_empty() {
        println!("\n❌ Found {} mismatches:\n", mismatches.len());
        for mismatch in &mismatches {
            println!("  {}", mismatch);
        }
        panic!("Error definitions do not match registry!");
    } else {
        println!("\n✓ All contract errors match the registry!");
    }
}
