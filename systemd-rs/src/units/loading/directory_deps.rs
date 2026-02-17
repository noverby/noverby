//! Support for systemd directory-based dependencies (.wants/, .requires/),
//! drop-in override directories (.d/), template unit instantiation with
//! specifier resolution, and a minimal getty generator.

use log::{trace, warn};
use std::collections::HashMap;
use std::convert::TryInto;
use std::path::{Path, PathBuf};

use crate::units::{
    ParsedFile, ParsingErrorReason, Unit, UnitId, UnitIdKind, parse_file, parse_service,
    parse_slice, parse_socket, parse_target,
};

/// Represents a dependency relationship discovered from a `.wants/` or `.requires/` directory.
#[derive(Debug, Clone)]
pub struct DirectoryDependency {
    /// The unit that has the dependency (e.g., "multi-user.target")
    pub parent_unit: String,
    /// The unit that is wanted/required (e.g., "getty.target")
    pub child_unit: String,
    /// Whether this is a Wants (false) or Requires (true) dependency
    pub is_requires: bool,
}

/// Check if a directory name represents a `.wants` or `.requires` dependency directory.
/// Returns Some((parent_unit_name, is_requires)) if it does.
#[allow(clippy::manual_map)]
pub fn parse_dep_dir_name(dir_name: &str) -> Option<(String, bool)> {
    if let Some(parent) = dir_name.strip_suffix(".wants") {
        Some((parent.to_owned(), false))
    } else if let Some(parent) = dir_name.strip_suffix(".requires") {
        Some((parent.to_owned(), true))
    } else {
        None
    }
}

/// Check if a directory name represents a drop-in override directory.
/// Returns Some(unit_name) if it does (e.g., "getty@.service.d" -> "getty@.service").
pub fn parse_dropin_dir_name(dir_name: &str) -> Option<String> {
    dir_name.strip_suffix(".d").map(|s| s.to_owned())
}

/// Determine if a filename is a unit file we can parse.
pub fn is_unit_file(filename: &str) -> bool {
    filename.ends_with(".service")
        || filename.ends_with(".socket")
        || filename.ends_with(".target")
        || filename.ends_with(".slice")
}

/// Check if a unit name is a template (e.g., "getty@.service", "modprobe@.service").
/// Template units contain `@.` before the extension and should never be activated directly —
/// only concrete instances (e.g., "getty@tty1.service") should be.
pub fn is_template_unit(name: &str) -> bool {
    // Find the '@' and then check if the next char before the extension dot is '.'
    // i.e., the instance part is empty: "foo@.service"
    if let Some(at_pos) = name.find('@') {
        if let Some(dot_pos) = name.rfind('.') {
            if dot_pos > at_pos {
                let instance = &name[at_pos + 1..dot_pos];
                return instance.is_empty();
            }
        }
    }
    false
}

/// Resolve symlink aliases among loaded units.
///
/// When a unit file is a symlink to another unit file (e.g., `default.target` → `multi-user.target`),
/// both get loaded as separate units with the same content.  This means `.wants/` directories
/// keyed by the *real* name (e.g., `multi-user.target.wants/`) are not associated with the
/// alias name (`default.target`).
///
/// This function detects such symlink relationships and merges the real unit's dependencies
/// into the alias unit, ensuring `.wants/` and `.requires/` directories for either name
/// apply to both.
pub fn resolve_symlink_aliases(
    unit_table: &mut HashMap<UnitId, Unit>,
    _dir_deps: &[DirectoryDependency],
    unit_dirs: &[PathBuf],
) {
    // Build a mapping: filename -> symlink target filename (if it's a symlink to another unit)
    let mut alias_map: HashMap<String, String> = HashMap::new(); // alias_name -> real_name

    for dir in unit_dirs {
        let entries = match std::fs::read_dir(dir) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let file_name = entry.file_name().to_string_lossy().to_string();
            if !is_unit_file(&file_name) {
                continue;
            }
            let path = entry.path();
            // Check if this is a symlink
            if let Ok(metadata) = std::fs::symlink_metadata(&path) {
                if metadata.file_type().is_symlink() {
                    if let Ok(target) = std::fs::read_link(&path) {
                        let target_name = target
                            .file_name()
                            .map(|f| f.to_string_lossy().to_string())
                            .unwrap_or_default();
                        if is_unit_file(&target_name) && target_name != file_name {
                            trace!("Symlink alias detected: {} -> {}", file_name, target_name);
                            alias_map.insert(file_name, target_name);
                        }
                    }
                }
            }
        }
    }

    if alias_map.is_empty() {
        return;
    }

    // For each alias relationship, merge the real unit's Wants/Requires (from dir_deps)
    // into the alias unit, and vice versa.
    // Also merge any deps that the real unit has from dir_deps into the alias unit directly.
    for (alias_name, real_name) in &alias_map {
        let alias_id = unit_table.keys().find(|id| id.name == *alias_name).cloned();
        let real_id = unit_table.keys().find(|id| id.name == *real_name).cloned();

        if let (Some(alias_id), Some(real_id)) = (alias_id, real_id) {
            // Collect deps from the real unit that the alias doesn't have
            let real_wants: Vec<UnitId> = unit_table
                .get(&real_id)
                .map(|u| u.common.dependencies.wants.clone())
                .unwrap_or_default();
            let real_requires: Vec<UnitId> = unit_table
                .get(&real_id)
                .map(|u| u.common.dependencies.requires.clone())
                .unwrap_or_default();
            let real_after: Vec<UnitId> = unit_table
                .get(&real_id)
                .map(|u| u.common.dependencies.after.clone())
                .unwrap_or_default();
            let real_before: Vec<UnitId> = unit_table
                .get(&real_id)
                .map(|u| u.common.dependencies.before.clone())
                .unwrap_or_default();
            let real_refs: Vec<UnitId> = unit_table
                .get(&real_id)
                .map(|u| u.common.unit.refs_by_name.clone())
                .unwrap_or_default();

            if let Some(alias_unit) = unit_table.get_mut(&alias_id) {
                for dep in &real_wants {
                    if !alias_unit.common.dependencies.wants.contains(dep) {
                        trace!(
                            "Alias merge: {} inherits Wants={} from {}",
                            alias_name, dep.name, real_name
                        );
                        alias_unit.common.dependencies.wants.push(dep.clone());
                    }
                }
                for dep in &real_requires {
                    if !alias_unit.common.dependencies.requires.contains(dep) {
                        trace!(
                            "Alias merge: {} inherits Requires={} from {}",
                            alias_name, dep.name, real_name
                        );
                        alias_unit.common.dependencies.requires.push(dep.clone());
                    }
                }
                for dep in &real_after {
                    if !alias_unit.common.dependencies.after.contains(dep) {
                        alias_unit.common.dependencies.after.push(dep.clone());
                    }
                }
                for dep in &real_before {
                    if !alias_unit.common.dependencies.before.contains(dep) {
                        alias_unit.common.dependencies.before.push(dep.clone());
                    }
                }
                for dep in &real_refs {
                    if !alias_unit.common.unit.refs_by_name.contains(dep) {
                        alias_unit.common.unit.refs_by_name.push(dep.clone());
                    }
                }
            }

            // Also merge alias's deps into the real unit (e.g., default.target.wants/ into multi-user.target)
            let alias_wants: Vec<UnitId> = unit_table
                .get(&alias_id)
                .map(|u| u.common.dependencies.wants.clone())
                .unwrap_or_default();
            let alias_requires: Vec<UnitId> = unit_table
                .get(&alias_id)
                .map(|u| u.common.dependencies.requires.clone())
                .unwrap_or_default();
            let alias_refs: Vec<UnitId> = unit_table
                .get(&alias_id)
                .map(|u| u.common.unit.refs_by_name.clone())
                .unwrap_or_default();

            if let Some(real_unit) = unit_table.get_mut(&real_id) {
                for dep in &alias_wants {
                    if !real_unit.common.dependencies.wants.contains(dep) {
                        real_unit.common.dependencies.wants.push(dep.clone());
                    }
                }
                for dep in &alias_requires {
                    if !real_unit.common.dependencies.requires.contains(dep) {
                        real_unit.common.dependencies.requires.push(dep.clone());
                    }
                }
                for dep in &alias_refs {
                    if !real_unit.common.unit.refs_by_name.contains(dep) {
                        real_unit.common.unit.refs_by_name.push(dep.clone());
                    }
                }
            }
        }
    }
}

/// Insert a parsed unit into the appropriate table based on the file extension.
pub fn insert_parsed_unit(
    services: &mut HashMap<UnitId, Unit>,
    sockets: &mut HashMap<UnitId, Unit>,
    targets: &mut HashMap<UnitId, Unit>,
    slices: &mut HashMap<UnitId, Unit>,
    parsed_file: ParsedFile,
    path: &PathBuf,
) {
    let path_str = path.to_str().unwrap_or("");

    if path_str.ends_with(".service") {
        trace!("Service found: {:?}", path);
        match parse_service(parsed_file, path).and_then(|parsed| {
            TryInto::<Unit>::try_into(parsed).map_err(ParsingErrorReason::Generic)
        }) {
            Ok(unit) => {
                services.insert(unit.id.clone(), unit);
            }
            Err(e) => {
                warn!("Skipping service {:?}: {:?}", path, e);
            }
        }
    } else if path_str.ends_with(".socket") {
        trace!("Socket found: {:?}", path);
        match parse_socket(parsed_file, path).and_then(|parsed| {
            TryInto::<Unit>::try_into(parsed).map_err(ParsingErrorReason::Generic)
        }) {
            Ok(unit) => {
                sockets.insert(unit.id.clone(), unit);
            }
            Err(e) => {
                warn!("Skipping socket {:?}: {:?}", path, e);
            }
        }
    } else if path_str.ends_with(".target") {
        trace!("Target found: {:?}", path);
        match parse_target(parsed_file, path).and_then(|parsed| {
            TryInto::<Unit>::try_into(parsed).map_err(ParsingErrorReason::Generic)
        }) {
            Ok(unit) => {
                targets.insert(unit.id.clone(), unit);
            }
            Err(e) => {
                warn!("Skipping target {:?}: {:?}", path, e);
            }
        }
    } else if path_str.ends_with(".slice") {
        trace!("Slice found: {:?}", path);
        match parse_slice(parsed_file, path).and_then(|parsed| {
            TryInto::<Unit>::try_into(parsed).map_err(ParsingErrorReason::Generic)
        }) {
            Ok(unit) => {
                slices.insert(unit.id.clone(), unit);
            }
            Err(e) => {
                warn!("Skipping slice {:?}: {:?}", path, e);
            }
        }
    }
}

/// Collect entries from a `.wants/` or `.requires/` directory and record
/// the dependency relationships.
pub fn collect_dep_dir_entries(
    dir_path: &Path,
    parent_unit: &str,
    is_requires: bool,
    dir_deps: &mut Vec<DirectoryDependency>,
) {
    let entries = match std::fs::read_dir(dir_path) {
        Ok(entries) => entries,
        Err(e) => {
            warn!("Could not read dependency directory {:?}: {}", dir_path, e);
            return;
        }
    };

    for entry in entries.flatten() {
        let child_name = entry.file_name().to_string_lossy().to_string();
        if is_unit_file(&child_name) {
            trace!(
                "Directory dependency: {} {} {}",
                parent_unit,
                if is_requires { "Requires" } else { "Wants" },
                child_name
            );
            dir_deps.push(DirectoryDependency {
                parent_unit: parent_unit.to_owned(),
                child_unit: child_name,
                is_requires,
            });
        }
    }
}

/// Collect entries from a `.d/` drop-in directory and store their contents.
pub fn collect_dropin_entries(
    dir_path: &Path,
    unit_name: &str,
    dropins: &mut HashMap<String, Vec<(String, String)>>,
) {
    let mut entries: Vec<_> = match std::fs::read_dir(dir_path) {
        Ok(entries) => entries.flatten().collect(),
        Err(e) => {
            warn!("Could not read drop-in directory {:?}: {}", dir_path, e);
            return;
        }
    };
    // Sort by filename for deterministic override ordering
    entries.sort_by_key(|e| e.file_name());

    for entry in entries {
        let filename = entry.file_name().to_string_lossy().to_string();
        if !filename.ends_with(".conf") {
            continue;
        }
        let content = match std::fs::read_to_string(entry.path()) {
            Ok(c) => c,
            Err(e) => {
                warn!("Could not read drop-in {:?}: {}", entry.path(), e);
                continue;
            }
        };
        trace!("Drop-in for {}: {:?}", unit_name, entry.path());
        dropins
            .entry(unit_name.to_owned())
            .or_default()
            .push((filename, content));
    }
}

/// Apply drop-in overrides to already-loaded units.
///
/// For each unit that has drop-in overrides, we find the base unit file,
/// merge the drop-in content, re-parse, and replace the unit in the table.
///
/// Drop-in merging follows systemd semantics:
/// - Sections from drop-ins are merged into the base file
/// - Settings in drop-ins override settings in the base file
/// - If a drop-in sets a list-type value to empty (e.g., `ExecStart=`),
///   it clears the base value before adding new entries
pub fn apply_dropins(
    units: &mut HashMap<UnitId, Unit>,
    dropins: &HashMap<String, Vec<(String, String)>>,
    unit_dirs: &[PathBuf],
) {
    let unit_names: Vec<String> = units.keys().map(|id| id.name.clone()).collect();

    for unit_name in unit_names {
        let overrides = match dropins.get(&unit_name) {
            Some(o) if !o.is_empty() => o,
            _ => continue,
        };

        // Find the unit in the table
        let unit_id = match units.keys().find(|id| id.name == unit_name) {
            Some(id) => id.clone(),
            None => continue,
        };

        // Get the base content from the unit's source path
        let base_path = match find_unit_file_in_dirs(unit_dirs, &unit_name) {
            Some(p) => p,
            None => continue,
        };

        let base_content = match std::fs::read_to_string(&base_path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        // Merge base + all drop-in contents
        let merged = merge_unit_contents(&base_content, overrides);

        let parsed_file = match parse_file(&merged) {
            Ok(pf) => pf,
            Err(e) => {
                warn!("Failed to re-parse {} with drop-ins: {:?}", unit_name, e);
                continue;
            }
        };

        // Re-parse the merged unit
        let new_unit = if unit_name.ends_with(".service") {
            parse_service(parsed_file, &base_path)
                .and_then(|p| TryInto::<Unit>::try_into(p).map_err(ParsingErrorReason::Generic))
        } else if unit_name.ends_with(".socket") {
            parse_socket(parsed_file, &base_path)
                .and_then(|p| TryInto::<Unit>::try_into(p).map_err(ParsingErrorReason::Generic))
        } else if unit_name.ends_with(".target") {
            parse_target(parsed_file, &base_path)
                .and_then(|p| TryInto::<Unit>::try_into(p).map_err(ParsingErrorReason::Generic))
        } else if unit_name.ends_with(".slice") {
            parse_slice(parsed_file, &base_path)
                .and_then(|p| TryInto::<Unit>::try_into(p).map_err(ParsingErrorReason::Generic))
        } else {
            continue;
        };

        match new_unit {
            Ok(unit) => {
                trace!("Applied drop-in overrides to {}", unit_name);
                units.remove(&unit_id);
                units.insert(unit.id.clone(), unit);
            }
            Err(e) => {
                warn!("Failed to apply drop-ins to {}: {:?}", unit_name, e);
            }
        }
    }
}

/// Apply directory-based dependencies to the unit table.
/// For each .wants/ and .requires/ relationship discovered, add the appropriate
/// dependency to the parent unit.
pub fn apply_directory_dependencies(
    unit_table: &mut HashMap<UnitId, Unit>,
    dir_deps: &[DirectoryDependency],
) {
    for dep in dir_deps {
        // Find the parent unit
        let parent_id = match unit_table.keys().find(|id| id.name == dep.parent_unit) {
            Some(id) => id.clone(),
            None => {
                trace!(
                    "Directory dependency: parent unit {} not found, skipping",
                    dep.parent_unit
                );
                continue;
            }
        };

        // The child unit might be a template instance (e.g., "serial-getty@ttyS0.service")
        // Check if it exists in the unit table
        let child_id: UnitId = match dep.child_unit.as_str().try_into() {
            Ok(id) => id,
            Err(_) => continue,
        };

        // Only add the dependency if the child unit exists (or will be instantiated later)
        let child_exists = unit_table.contains_key(&child_id);

        if child_exists {
            if let Some(parent) = unit_table.get_mut(&parent_id) {
                if dep.is_requires {
                    if !parent.common.dependencies.requires.contains(&child_id) {
                        trace!(
                            "Adding directory Requires dependency: {} -> {}",
                            dep.parent_unit, dep.child_unit
                        );
                        parent.common.dependencies.requires.push(child_id.clone());
                    }
                } else if !parent.common.dependencies.wants.contains(&child_id) {
                    trace!(
                        "Adding directory Wants dependency: {} -> {}",
                        dep.parent_unit, dep.child_unit
                    );
                    parent.common.dependencies.wants.push(child_id.clone());
                }
                if !parent.common.unit.refs_by_name.contains(&child_id) {
                    parent.common.unit.refs_by_name.push(child_id);
                }
            }
        } else {
            trace!(
                "Directory dependency: child unit {} not yet loaded (may be template instance)",
                dep.child_unit
            );
        }
    }
}

/// Extract the template name and instance from a unit name like "serial-getty@ttyS0.service".
/// Returns (template_name, instance) e.g., ("serial-getty@.service", "ttyS0").
pub fn parse_template_instance(unit_name: &str) -> Option<(String, String)> {
    let at_pos = unit_name.find('@')?;
    // Find the suffix (.service, .socket, etc.)
    let dot_pos = unit_name.rfind('.')?;
    if dot_pos <= at_pos {
        return None;
    }
    let instance = &unit_name[at_pos + 1..dot_pos];
    if instance.is_empty() {
        // This is a template itself (e.g., "getty@.service"), not an instance
        return None;
    }
    let template = format!("{}@{}", &unit_name[..at_pos], &unit_name[dot_pos..]);
    Some((template, instance.to_owned()))
}

/// Resolve systemd specifiers in a unit file content string.
/// Supports: %I, %i, %N, %n, %p, %P, %%
pub fn resolve_specifiers(content: &str, unit_name: &str, instance: &str) -> String {
    // %n = full unit name (e.g., "serial-getty@ttyS0.service")
    // %N = same as %n but with unescaping (simplified: same as %n)
    // %p = prefix (unit name without the suffix, e.g., "serial-getty@ttyS0")
    // %P = same as %p but unescaped
    // %i = instance name (e.g., "ttyS0")
    // %I = same as %i but unescaped
    let prefix = unit_name
        .rfind('.')
        .map(|pos| &unit_name[..pos])
        .unwrap_or(unit_name);

    let mut result = String::with_capacity(content.len());
    let mut chars = content.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '%' {
            match chars.peek() {
                Some(&'I') => {
                    chars.next();
                    result.push_str(instance);
                }
                Some(&'i') => {
                    chars.next();
                    result.push_str(instance);
                }
                Some(&'N') => {
                    chars.next();
                    result.push_str(unit_name);
                }
                Some(&'n') => {
                    chars.next();
                    result.push_str(unit_name);
                }
                Some(&'p') => {
                    chars.next();
                    result.push_str(prefix);
                }
                Some(&'P') => {
                    chars.next();
                    result.push_str(prefix);
                }
                Some(&'%') => {
                    chars.next();
                    result.push('%');
                }
                _ => {
                    // Unknown specifier — keep as-is
                    result.push('%');
                }
            }
        } else {
            result.push(c);
        }
    }

    result
}

/// Load a template unit, instantiate it with the given instance name,
/// and return the resulting Unit.
pub fn instantiate_template(
    template_name: &str,
    instance_name: &str,
    instance_unit_name: &str,
    unit_dirs: &[PathBuf],
    dropins: &HashMap<String, Vec<(String, String)>>,
) -> Option<Unit> {
    // Find the template file
    let template_path = find_unit_file_in_dirs(unit_dirs, template_name)?;
    let base_content = std::fs::read_to_string(&template_path).ok()?;

    // Get drop-ins for both the template and the instance
    let mut all_overrides: Vec<(String, String)> = Vec::new();
    if let Some(template_dropins) = dropins.get(template_name) {
        all_overrides.extend(template_dropins.iter().cloned());
    }
    if let Some(instance_dropins) = dropins.get(instance_unit_name) {
        all_overrides.extend(instance_dropins.iter().cloned());
    }

    // Merge base + drop-ins
    let merged = if all_overrides.is_empty() {
        base_content
    } else {
        merge_unit_contents(&base_content, &all_overrides)
    };

    // Resolve specifiers
    let resolved = resolve_specifiers(&merged, instance_unit_name, instance_name);

    // Parse
    let parsed_file = parse_file(&resolved).ok()?;

    // Create a fake path with the instance name for the parser
    let instance_path = template_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(instance_unit_name);

    // Parse and convert to Unit
    if instance_unit_name.ends_with(".service") {
        parse_service(parsed_file, &instance_path)
            .and_then(|p| TryInto::<Unit>::try_into(p).map_err(ParsingErrorReason::Generic))
            .ok()
    } else if instance_unit_name.ends_with(".socket") {
        parse_socket(parsed_file, &instance_path)
            .and_then(|p| TryInto::<Unit>::try_into(p).map_err(ParsingErrorReason::Generic))
            .ok()
    } else if instance_unit_name.ends_with(".target") {
        parse_target(parsed_file, &instance_path)
            .and_then(|p| TryInto::<Unit>::try_into(p).map_err(ParsingErrorReason::Generic))
            .ok()
    } else {
        None
    }
}

/// Instantiate template units that are referenced by directory dependencies
/// but not yet in the unit table.
pub fn instantiate_template_units(
    unit_table: &mut HashMap<UnitId, Unit>,
    dir_deps: &[DirectoryDependency],
    unit_dirs: &[PathBuf],
    dropins: &HashMap<String, Vec<(String, String)>>,
) {
    // Collect unique instance names that need to be instantiated
    // Each entry: (instance_unit_name, template_name, instance_name)
    let mut instances_to_create: Vec<(String, String, String)> = Vec::new();

    for dep in dir_deps {
        let child_id: UnitId = match dep.child_unit.as_str().try_into() {
            Ok(id) => id,
            Err(_) => continue,
        };

        // Skip if already loaded
        if unit_table.contains_key(&child_id) {
            continue;
        }

        // Check if this is a template instance
        if let Some((template_name, instance_name)) = parse_template_instance(&dep.child_unit)
            && !instances_to_create
                .iter()
                .any(|(n, _, _)| n == &dep.child_unit)
        {
            instances_to_create.push((dep.child_unit.clone(), template_name, instance_name));
        }
    }

    // Also check for template instances referenced in Wants=/Requires= of existing units
    let existing_refs: Vec<String> = unit_table
        .values()
        .flat_map(|u| {
            u.common
                .dependencies
                .wants
                .iter()
                .chain(u.common.dependencies.requires.iter())
                .map(|id| id.name.clone())
        })
        .collect();

    for ref_name in &existing_refs {
        let ref_id: UnitId = match ref_name.as_str().try_into() {
            Ok(id) => id,
            Err(_) => continue,
        };
        if unit_table.contains_key(&ref_id) {
            continue;
        }
        if let Some((template_name, instance_name)) = parse_template_instance(ref_name)
            && !instances_to_create.iter().any(|(n, _, _)| n == ref_name)
        {
            instances_to_create.push((ref_name.clone(), template_name, instance_name));
        }
    }

    // Instantiate each template
    for (instance_unit_name, template_name, instance_name) in &instances_to_create {
        trace!(
            "Instantiating template {} with instance {} -> {}",
            template_name, instance_name, instance_unit_name
        );

        if let Some(unit) = instantiate_template(
            template_name,
            instance_name,
            instance_unit_name,
            unit_dirs,
            dropins,
        ) {
            unit_table.insert(unit.id.clone(), unit);
            trace!("Successfully instantiated {}", instance_unit_name);
        } else {
            warn!(
                "Failed to instantiate template {} for instance {}",
                template_name, instance_unit_name
            );
        }
    }

    // Now re-apply directory dependencies for newly instantiated units
    for dep in dir_deps {
        let parent_id = match unit_table.keys().find(|id| id.name == dep.parent_unit) {
            Some(id) => id.clone(),
            None => continue,
        };

        let child_id: UnitId = match dep.child_unit.as_str().try_into() {
            Ok(id) => id,
            Err(_) => continue,
        };

        if !unit_table.contains_key(&child_id) {
            continue;
        }

        if let Some(parent) = unit_table.get_mut(&parent_id) {
            if dep.is_requires {
                if !parent.common.dependencies.requires.contains(&child_id) {
                    parent.common.dependencies.requires.push(child_id.clone());
                }
            } else if !parent.common.dependencies.wants.contains(&child_id) {
                parent.common.dependencies.wants.push(child_id.clone());
            }
            if !parent.common.unit.refs_by_name.contains(&child_id) {
                parent.common.unit.refs_by_name.push(child_id);
            }
        }
    }
}

/// Minimal implementation of systemd-getty-generator functionality.
/// Reads /proc/cmdline and creates serial-getty instances for kernel consoles.
pub fn generate_getty_units(
    unit_table: &mut HashMap<UnitId, Unit>,
    unit_dirs: &[PathBuf],
    dropins: &HashMap<String, Vec<(String, String)>>,
) {
    // Only run if /proc/cmdline exists (i.e., on a real Linux system)
    let cmdline = match std::fs::read_to_string("/proc/cmdline") {
        Ok(c) => c,
        Err(_) => return,
    };

    // Parse console= arguments from kernel command line
    let mut consoles: Vec<String> = Vec::new();
    for param in cmdline.split_whitespace() {
        if let Some(console) = param.strip_prefix("console=") {
            // console=ttyS0,115200 -> extract "ttyS0"
            let tty = console.split(',').next().unwrap_or(console);
            consoles.push(tty.to_owned());
        }
    }

    if consoles.is_empty() {
        return;
    }

    // Find the getty.target to add Wants dependencies
    let getty_target_id = UnitId {
        name: "getty.target".to_owned(),
        kind: UnitIdKind::Target,
    };

    for console in &consoles {
        // Only create serial-getty for serial consoles (ttyS*, ttyAMA*, ttyUSB*, etc.)
        // Skip virtual consoles (tty0, tty1, etc.)
        let is_serial = console.starts_with("ttyS")
            || console.starts_with("ttyAMA")
            || console.starts_with("ttyUSB")
            || console.starts_with("ttyACM")
            || console.starts_with("hvc");

        if !is_serial {
            continue;
        }

        let instance_name = format!("serial-getty@{}.service", console);
        let instance_id: UnitId = match instance_name.as_str().try_into() {
            Ok(id) => id,
            Err(_) => continue,
        };

        // Skip if already loaded
        if unit_table.contains_key(&instance_id) {
            continue;
        }

        // Try to instantiate from the serial-getty@.service template
        if let Some(unit) = instantiate_template(
            "serial-getty@.service",
            console,
            &instance_name,
            unit_dirs,
            dropins,
        ) {
            trace!("Getty generator: created {}", instance_name);
            unit_table.insert(unit.id.clone(), unit);

            // Add to getty.target's Wants
            if let Some(getty_target) = unit_table.get_mut(&getty_target_id) {
                if !getty_target
                    .common
                    .dependencies
                    .wants
                    .contains(&instance_id)
                {
                    getty_target
                        .common
                        .dependencies
                        .wants
                        .push(instance_id.clone());
                }
                if !getty_target.common.unit.refs_by_name.contains(&instance_id) {
                    getty_target.common.unit.refs_by_name.push(instance_id);
                }
            }
        } else {
            warn!(
                "Getty generator: could not instantiate serial-getty@.service for {}",
                console
            );
        }
    }
}

// ── Internal helpers ──────────────────────────────────────────────────────

/// Merge base unit file content with drop-in override contents.
fn merge_unit_contents(base: &str, overrides: &[(String, String)]) -> String {
    let mut result = base.to_owned();

    for (_filename, content) in overrides {
        result = merge_ini_sections(&result, content);
    }

    result
}

/// Merge two INI-format unit file contents, combining sections with the same name.
/// If both files have the same section, the settings from the override are appended
/// to the base section. If an override sets a key to empty value, it clears the key.
fn merge_ini_sections(base: &str, overlay: &str) -> String {
    let base_sections = parse_ini_to_sections(base);
    let overlay_sections = parse_ini_to_sections(overlay);

    // Start with base sections
    let mut merged_sections: Vec<(String, Vec<String>)> = Vec::new();
    let mut seen_sections: HashMap<String, usize> = HashMap::new();

    for (name, lines) in &base_sections {
        seen_sections.insert(name.clone(), merged_sections.len());
        merged_sections.push((name.clone(), lines.clone()));
    }

    // Merge overlay sections
    for (name, lines) in &overlay_sections {
        if let Some(&idx) = seen_sections.get(name) {
            // Merge into existing section
            for line in lines {
                let trimmed = line.trim();
                if let Some(pos) = trimmed.find('=') {
                    let value = trimmed[pos + 1..].trim();
                    if value.is_empty() {
                        // This is a reset: remove all previous lines with this key
                        let key = trimmed[..pos].trim();
                        merged_sections[idx].1.retain(|existing| {
                            let existing_trimmed = existing.trim();
                            if let Some(epos) = existing_trimmed.find('=') {
                                existing_trimmed[..epos].trim() != key
                            } else {
                                true
                            }
                        });
                        // Don't add the empty assignment itself
                        continue;
                    }
                }
                merged_sections[idx].1.push(line.clone());
            }
        } else {
            // New section from overlay
            seen_sections.insert(name.clone(), merged_sections.len());
            merged_sections.push((name.clone(), lines.clone()));
        }
    }

    // Reconstruct the INI content
    let mut result = String::new();
    for (name, lines) in &merged_sections {
        result.push_str(name);
        result.push('\n');
        for line in lines {
            result.push_str(line);
            result.push('\n');
        }
        result.push('\n');
    }

    result
}

/// Parse INI content into a list of (section_name, lines) pairs.
fn parse_ini_to_sections(content: &str) -> Vec<(String, Vec<String>)> {
    let mut sections = Vec::new();
    let mut current_section = String::new();
    let mut current_lines = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            if !current_section.is_empty() {
                sections.push((current_section.clone(), current_lines.clone()));
                current_lines.clear();
            }
            current_section = trimmed.to_owned();
        } else if !current_section.is_empty() {
            // Skip empty lines and comments
            if !trimmed.is_empty() && !trimmed.starts_with('#') && !trimmed.starts_with(';') {
                current_lines.push(trimmed.to_owned());
            }
        }
    }

    if !current_section.is_empty() {
        sections.push((current_section, current_lines));
    }

    sections
}

/// Find a unit file by name across all unit directories.
fn find_unit_file_in_dirs(unit_dirs: &[PathBuf], unit_name: &str) -> Option<PathBuf> {
    for dir in unit_dirs {
        let candidate = dir.join(unit_name);
        if candidate.exists() && !candidate.is_dir() {
            return Some(candidate);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_dep_dir_name() {
        assert_eq!(
            parse_dep_dir_name("multi-user.target.wants"),
            Some(("multi-user.target".to_owned(), false))
        );
        assert_eq!(
            parse_dep_dir_name("sysinit.target.requires"),
            Some(("sysinit.target".to_owned(), true))
        );
        assert_eq!(parse_dep_dir_name("some-directory"), None);
    }

    #[test]
    fn test_parse_dropin_dir_name() {
        assert_eq!(
            parse_dropin_dir_name("getty@.service.d"),
            Some("getty@.service".to_owned())
        );
        assert_eq!(
            parse_dropin_dir_name("sshd.service.d"),
            Some("sshd.service".to_owned())
        );
        assert_eq!(parse_dropin_dir_name("some-directory"), None);
    }

    #[test]
    fn test_parse_template_instance() {
        assert_eq!(
            parse_template_instance("serial-getty@ttyS0.service"),
            Some(("serial-getty@.service".to_owned(), "ttyS0".to_owned()))
        );
        assert_eq!(
            parse_template_instance("getty@tty1.service"),
            Some(("getty@.service".to_owned(), "tty1".to_owned()))
        );
        // Template itself (no instance)
        assert_eq!(parse_template_instance("getty@.service"), None);
        // No @ at all
        assert_eq!(parse_template_instance("sshd.service"), None);
    }

    #[test]
    fn test_resolve_specifiers() {
        let result = resolve_specifiers(
            "TTYPath=/dev/%I\nDescription=Getty on %I\nUtmpIdentifier=%I",
            "serial-getty@ttyS0.service",
            "ttyS0",
        );
        assert_eq!(
            result,
            "TTYPath=/dev/ttyS0\nDescription=Getty on ttyS0\nUtmpIdentifier=ttyS0"
        );
    }

    #[test]
    fn test_resolve_specifiers_all() {
        let result = resolve_specifiers("%n %N %i %I %p %P %%", "foo@bar.service", "bar");
        assert_eq!(
            result,
            "foo@bar.service foo@bar.service bar bar foo@bar foo@bar %"
        );
    }

    #[test]
    fn test_merge_ini_sections_basic() {
        let base = "[Unit]\nDescription=Base\n\n[Service]\nExecStart=/bin/true\n";
        let overlay = "[Service]\nEnvironment=FOO=bar\n";
        let merged = merge_ini_sections(base, overlay);

        assert!(merged.contains("Description=Base"));
        assert!(merged.contains("ExecStart=/bin/true"));
        assert!(merged.contains("Environment=FOO=bar"));
    }

    #[test]
    fn test_merge_ini_sections_reset() {
        let base = "[Service]\nExecStart=/bin/old\n";
        let overlay = "[Service]\nExecStart=\nExecStart=/bin/new\n";
        let merged = merge_ini_sections(base, overlay);

        assert!(!merged.contains("/bin/old"));
        assert!(merged.contains("ExecStart=/bin/new"));
    }

    #[test]
    fn test_merge_ini_sections_new_section() {
        let base = "[Unit]\nDescription=Test\n";
        let overlay = "[Install]\nWantedBy=multi-user.target\n";
        let merged = merge_ini_sections(base, overlay);

        assert!(merged.contains("[Unit]"));
        assert!(merged.contains("Description=Test"));
        assert!(merged.contains("[Install]"));
        assert!(merged.contains("WantedBy=multi-user.target"));
    }

    #[test]
    fn test_is_unit_file() {
        assert!(is_unit_file("sshd.service"));
        assert!(is_unit_file("sshd.socket"));
        assert!(is_unit_file("multi-user.target"));
        assert!(is_unit_file("user.slice"));
        assert!(!is_unit_file("override.conf"));
        assert!(!is_unit_file("README.md"));
    }

    #[test]
    fn test_is_template_unit() {
        // Templates have an empty instance between '@' and the extension
        assert!(is_template_unit("getty@.service"));
        assert!(is_template_unit("serial-getty@.service"));
        assert!(is_template_unit("modprobe@.service"));
        assert!(is_template_unit("container-getty@.service"));
        assert!(is_template_unit("systemd-backlight@.service"));
        assert!(is_template_unit("autovt@.service"));
        assert!(is_template_unit("capsule@.service"));

        // Concrete instances are NOT templates
        assert!(!is_template_unit("getty@tty1.service"));
        assert!(!is_template_unit("serial-getty@ttyS0.service"));
        assert!(!is_template_unit("modprobe@dm_mod.service"));
        assert!(!is_template_unit("autovt@tty1.service"));

        // Regular units are NOT templates
        assert!(!is_template_unit("sshd.service"));
        assert!(!is_template_unit("multi-user.target"));
        assert!(!is_template_unit("dbus.socket"));
        assert!(!is_template_unit("user.slice"));

        // Edge cases
        assert!(!is_template_unit("no-extension"));
        assert!(!is_template_unit(""));
    }
}
