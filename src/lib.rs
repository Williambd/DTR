use sha1::Digest;
use std::fs;
use std::path::Path;

/// The .dtr directory name
const DTR_DIR: &str = ".dtr";

/// Supported node types
pub const NODE_TYPE_INPUT: &str = "input";
pub const NODE_TYPE_PROCESS: &str = "process";
pub const NODE_TYPE_CHART: &str = "chart";
pub const NODE_TYPE_MODEL: &str = "model";
pub const NODE_TYPE_MERGE: &str = "merge";

/// Node metadata stored as JSON in the nodes/ directory.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Node {
    pub node_type: String,
    pub parents: Vec<String>,
    /// Variable names for each parent (empty string = pipes via |>).
    /// Index-aligned with `parents`. Stored at creation time so compose
    /// uses the exact names the merge code was written against.
    #[serde(default)]
    pub parent_vars: Vec<String>,
    pub children: Vec<String>,
    pub blob: String,
    pub cache: Option<String>,
}

/// Errors that can occur during DTR operations.
#[derive(Debug)]
pub enum DtrError {
    Io(std::io::Error),
    NoCurrentNode,
    InvalidNodeType(String),
    NodeNotFound(String),
    InvalidState(String),
    RExecError(String),
    Json(serde_json::Error),
}

impl std::fmt::Display for DtrError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DtrError::Io(e) => write!(f, "I/O error: {e}"),
            DtrError::NoCurrentNode => write!(f, "no current working node (CWN is empty)"),
            DtrError::InvalidNodeType(t) => write!(f, "invalid node type: {t}"),
            DtrError::NodeNotFound(h) => write!(f, "node not found: {h}"),
            DtrError::InvalidState(s) => write!(f, "invalid state: {s}"),
            DtrError::RExecError(s) => write!(f, "R error:\n{s}"),
            DtrError::Json(e) => write!(f, "JSON error: {e}"),
        }
    }
}

impl From<std::io::Error> for DtrError {
    fn from(e: std::io::Error) -> Self {
        DtrError::Io(e)
    }
}

impl From<serde_json::Error> for DtrError {
    fn from(e: serde_json::Error) -> Self {
        DtrError::Json(e)
    }
}

/// Initialize a DTR project in the given directory.
/// Creates the `.dtr` directory structure with empty blobs/, nodes/, cache/
/// directories, an empty CWN file, a next_id counter,
/// an empty markers JSON object, and an empty packages list.
pub fn init(dir: &Path) -> Result<(), DtrError> {
    let dtr_path = dir.join(DTR_DIR);

    fs::create_dir_all(dtr_path.join("blobs"))?;
    fs::create_dir_all(dtr_path.join("nodes"))?;
    fs::create_dir_all(dtr_path.join("cache"))?;

    let cwn_path = dtr_path.join("CWN");
    if !cwn_path.exists() {
        fs::write(&cwn_path, "")?;
    }

    let next_id_path = dtr_path.join("next_id");
    if !next_id_path.exists() {
        fs::write(&next_id_path, "0")?;
    }

    let markers_path = dtr_path.join("markers");
    if !markers_path.exists() {
        fs::write(&markers_path, "{}\n")?;
    }

    let packages_path = dtr_path.join("packages");
    if !packages_path.exists() {
        fs::write(&packages_path, "[]\n")?;
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

fn dtr_path(dir: &Path) -> std::path::PathBuf {
    dir.join(DTR_DIR)
}

fn hash_string(s: &str) -> String {
    let mut hasher = sha1::Sha1::new();
    hasher.update(s.as_bytes());
    hex::encode(hasher.finalize())
}

fn hash_bytes(data: &[u8]) -> String {
    let mut hasher = sha1::Sha1::new();
    hasher.update(data);
    hex::encode(hasher.finalize())
}

fn cwn_read(dir: &Path) -> Result<String, DtrError> {
    let content = fs::read_to_string(dtr_path(dir).join("CWN"))?;
    let trimmed = content.trim().to_string();
    if trimmed.is_empty() {
        Err(DtrError::NoCurrentNode)
    } else {
        Ok(trimmed)
    }
}

fn cwn_write(dir: &Path, hash: &str) -> Result<(), DtrError> {
    fs::write(dtr_path(dir).join("CWN"), format!("{hash}\n"))?;
    Ok(())
}

fn node_read(dir: &Path, hash: &str) -> Result<Node, DtrError> {
    let path = dtr_path(dir).join("nodes").join(hash);
    let content = fs::read_to_string(&path).map_err(|_| DtrError::NodeNotFound(hash.to_string()))?;
    let node: Node = serde_json::from_str(&content)?;
    Ok(node)
}

fn node_write(dir: &Path, hash: &str, node: &Node) -> Result<(), DtrError> {
    let content = serde_json::to_string_pretty(node)?;
    fs::write(dtr_path(dir).join("nodes").join(hash), &content)?;
    Ok(())
}

fn blob_write(dir: &Path, code: &str) -> Result<String, DtrError> {
    let hash = hash_string(code);
    let path = dtr_path(dir).join("blobs").join(&hash);
    if !path.exists() {
        fs::write(&path, code)?;
    }
    Ok(hash)
}

fn markers_read(dir: &Path) -> Result<serde_json::Map<String, serde_json::Value>, DtrError> {
    let content = fs::read_to_string(dtr_path(dir).join("markers"))?;
    let map: serde_json::Map<String, serde_json::Value> = serde_json::from_str(&content)?;
    Ok(map)
}

fn markers_write(dir: &Path, map: &serde_json::Map<String, serde_json::Value>) -> Result<(), DtrError> {
    let content = serde_json::to_string_pretty(map)?;
    fs::write(dtr_path(dir).join("markers"), content + "\n")?;
    Ok(())
}

fn packages_read(dir: &Path) -> Result<Vec<String>, DtrError> {
    let content = fs::read_to_string(dtr_path(dir).join("packages"))?;
    let packages: Vec<String> = serde_json::from_str(&content)?;
    Ok(packages)
}

fn packages_write(dir: &Path, packages: &[String]) -> Result<(), DtrError> {
    let content = serde_json::to_string_pretty(packages)?;
    fs::write(dtr_path(dir).join("packages"), content + "\n")?;
    Ok(())
}

fn library_imports(dir: &Path) -> Result<String, DtrError> {
    let packages = packages_read(dir)?;
    if packages.is_empty() {
        return Ok(String::new());
    }
    Ok(packages
        .iter()
        .map(|p| format!("library({p})"))
        .collect::<Vec<_>>()
        .join("\n")
        + "\n\n")
}

fn auto_marker_name(dir: &Path, prefix: &str) -> Result<String, DtrError> {
    let markers = markers_read(dir)?;
    let mut i = 1;
    loop {
        let name = format!("{prefix}_{i}");
        if !markers.contains_key(&name) {
            return Ok(name);
        }
        i += 1;
    }
}

fn next_node_id(dir: &Path) -> Result<String, DtrError> {
    let path = dtr_path(dir).join("next_id");
    let content = fs::read_to_string(&path)?;
    let id: u64 = content.trim().parse().map_err(|_| {
        DtrError::InvalidState("corrupt next_id file".to_string())
    })?;
    fs::write(&path, format!("{}", id + 1))?;
    Ok(format!("n{:x}", id))
}

// ---------------------------------------------------------------------------
// Public API: add
// ---------------------------------------------------------------------------

/// Add a child node to the current working node.
/// Returns the hash of the newly created node.
/// If `marker` is provided, creates a marker pointing to the new node.
pub fn add(dir: &Path, node_type: &str, code: &str, marker: Option<&str>) -> Result<String, DtrError> {
    validate_node_type(node_type)?;
    let current = cwn_read(dir)?;
    let hash = add_child_to(dir, &current, node_type, code, &[])?;
    if let Some(name) = marker {
        set_marker(dir, &hash, name)?;
    }
    Ok(hash)
}

/// Add an input node. Creates a root node (no parent) and auto-generates a
/// marker unless one is provided via `marker`.
pub fn add_input(dir: &Path, code: &str, marker: Option<&str>) -> Result<String, DtrError> {
    validate_node_type(NODE_TYPE_INPUT)?;
    let node_hash = create_node(dir, NODE_TYPE_INPUT, code, &[], &[])?;

    let marker_name = match marker {
        Some(name) => name.to_string(),
        None => auto_marker_name(dir, "input")?,
    };
    set_marker(dir, &node_hash, &marker_name)?;

    cwn_write(dir, &node_hash)?;
    Ok(node_hash)
}

/// Add a merge node with multiple parents.
/// `parents` can be node hashes or marker names.
/// If `marker` is provided, creates a marker pointing to the new node.
pub fn add_merge(dir: &Path, code: &str, parents: &[&str], marker: Option<&str>) -> Result<String, DtrError> {
    validate_node_type(NODE_TYPE_MERGE)?;
    if parents.len() < 2 {
        return Err(DtrError::InvalidState(
            "merge node requires at least 2 parents".to_string(),
        ));
    }
    // Resolve all parent refs (marker names or hashes) and verify they exist
    let resolved: Vec<String> = parents
        .iter()
        .map(|p| resolve_ref(dir, p))
        .collect::<Result<_, _>>()?;
    for p in &resolved {
        node_read(dir, p)?;
    }

    // Derive variable names for side parents at creation time
    let markers = markers_read(dir).ok();
    let vars: Vec<String> = resolved
        .iter()
        .enumerate()
        .map(|(i, p)| {
            if i == 0 {
                String::new()
            } else {
                derive_parent_var_from(markers.as_ref(), p)
            }
        })
        .collect();

    let parent_refs: Vec<&str> = resolved.iter().map(|s| s.as_str()).collect();
    let node_hash = create_node(dir, NODE_TYPE_MERGE, code, &parent_refs, &vars)?;

    // Add this node as a child to each parent
    for p in &resolved {
        let mut parent = node_read(dir, p)?;
        if !parent.children.contains(&node_hash) {
            parent.children.push(node_hash.clone());
            node_write(dir, p, &parent)?;
        }
    }

    if let Some(name) = marker {
        set_marker(dir, &node_hash, name)?;
    }
    cwn_write(dir, &node_hash)?;
    Ok(node_hash)
}

/// Set a marker name pointing to a node hash.
fn set_marker(dir: &Path, hash: &str, name: &str) -> Result<(), DtrError> {
    let mut markers = markers_read(dir)?;
    markers.insert(name.to_string(), serde_json::Value::String(hash.to_string()));
    markers_write(dir, &markers)?;
    Ok(())
}

/// Resolve a reference string to a node hash.
/// Tries marker name first, falls back to treating the string as a direct
/// node hash.
fn resolve_ref(dir: &Path, ref_str: &str) -> Result<String, DtrError> {
    // Try as a marker name
    if let Ok(markers) = markers_read(dir) {
        if let Some(val) = markers.get(ref_str) {
            if let Some(hash) = val.as_str() {
                let path = dtr_path(dir).join("nodes").join(hash);
                if path.exists() {
                    return Ok(hash.to_string());
                }
            }
        }
    }
    // Try as a direct node hash
    let path = dtr_path(dir).join("nodes").join(ref_str);
    if path.exists() {
        return Ok(ref_str.to_string());
    }
    Err(DtrError::NodeNotFound(ref_str.to_string()))
}

// ---------------------------------------------------------------------------
// Internal: node creation
// ---------------------------------------------------------------------------

fn validate_node_type(node_type: &str) -> Result<(), DtrError> {
    match node_type {
        NODE_TYPE_INPUT | NODE_TYPE_PROCESS | NODE_TYPE_CHART | NODE_TYPE_MODEL | NODE_TYPE_MERGE => Ok(()),
        other => Err(DtrError::InvalidNodeType(other.to_string())),
    }
}

fn add_child_to(
    dir: &Path,
    parent_hash: &str,
    node_type: &str,
    code: &str,
    extra_parents: &[&str],
) -> Result<String, DtrError> {
    // Verify parent exists
    node_read(dir, parent_hash)?;

    let mut all_parents: Vec<String> = vec![parent_hash.to_string()];
    for p in extra_parents {
        all_parents.push(p.to_string());
    }
    let parent_refs: Vec<&str> = all_parents.iter().map(|s| s.as_str()).collect();

    let parent_vars = vec!["".to_string()];
    let node_hash = create_node(dir, node_type, code, &parent_refs, &parent_vars)?;

    // Add this node as a child to the primary parent
    let mut parent = node_read(dir, parent_hash)?;
    if !parent.children.contains(&node_hash) {
        parent.children.push(node_hash.clone());
        node_write(dir, parent_hash, &parent)?;
    }

    cwn_write(dir, &node_hash)?;
    Ok(node_hash)
}

fn create_node(
    dir: &Path,
    node_type: &str,
    code: &str,
    parents: &[&str],
    parent_vars: &[String],
) -> Result<String, DtrError> {
    let blob_hash = blob_write(dir, code)?;

    let node = Node {
        node_type: node_type.to_string(),
        parents: parents.iter().map(|s| s.to_string()).collect(),
        parent_vars: parent_vars.to_vec(),
        children: Vec::new(),
        blob: blob_hash,
        cache: None,
    };

    let new_id = next_node_id(dir)?;
    let json = serde_json::to_string(&node)?;
    fs::write(dtr_path(dir).join("nodes").join(&new_id), &json)?;

    Ok(new_id)
}

// ---------------------------------------------------------------------------
// Public API: goto
// ---------------------------------------------------------------------------

/// Move the current working node to the given target.
///
/// Targets:
/// - `".."` — move to the single parent (errors if multiple or none)
/// - `"."`  — move to the single child (errors if multiple or none)
/// - any other string — resolved as a marker name or node hash
pub fn goto(dir: &Path, target: &str) -> Result<(), DtrError> {
    match target {
        ".." => goto_parent(dir),
        "." => goto_child(dir),
        other => goto_ref(dir, other),
    }
}

fn goto_parent(dir: &Path) -> Result<(), DtrError> {
    let hash = cwn_read(dir)?;
    let node = node_read(dir, &hash)?;
    match node.parents.len() {
        0 => Err(DtrError::InvalidState(
            "no parent (current node is a root)".to_string(),
        )),
        1 => cwn_write(dir, &node.parents[0]),
        _ => Err(DtrError::InvalidState(format!(
            "multiple parents ({}); use an explicit hash or marker to choose",
            node.parents.len()
        ))),
    }
}

fn goto_child(dir: &Path) -> Result<(), DtrError> {
    let hash = cwn_read(dir)?;
    let node = node_read(dir, &hash)?;
    match node.children.len() {
        0 => Err(DtrError::InvalidState(
            "no child (current node is a leaf)".to_string(),
        )),
        1 => cwn_write(dir, &node.children[0]),
        _ => Err(DtrError::InvalidState(format!(
            "multiple children ({}); use an explicit hash or marker to choose",
            node.children.len()
        ))),
    }
}

fn goto_ref(dir: &Path, ref_str: &str) -> Result<(), DtrError> {
    let hash = resolve_ref(dir, ref_str)?;
    cwn_write(dir, &hash)
}

// ---------------------------------------------------------------------------
// Public API: read helpers for testing
// ---------------------------------------------------------------------------

/// Read the blob (R code) for a given blob hash.
pub fn read_blob(dir: &Path, hash: &str) -> Result<String, DtrError> {
    let path = dtr_path(dir).join("blobs").join(hash);
    Ok(fs::read_to_string(&path)?)
}

/// Read the current working node hash.
pub fn read_cwn(dir: &Path) -> Result<String, DtrError> {
    cwn_read(dir)
}

/// Read a node's metadata.
pub fn read_node(dir: &Path, hash: &str) -> Result<Node, DtrError> {
    node_read(dir, hash)
}

/// Read the R code of the current working node.
pub fn read_current(dir: &Path) -> Result<String, DtrError> {
    let node_hash = cwn_read(dir)?;
    let node = node_read(dir, &node_hash)?;
    let path = dtr_path(dir).join("blobs").join(&node.blob);
    let code = fs::read_to_string(&path).map_err(|_| {
        DtrError::InvalidState(format!("blob not found: {}", node.blob))
    })?;
    Ok(code)
}

/// Replace the R code of the current working node.
/// Returns the hash of the updated node (may differ from old hash since
/// node content changed). Updates parent/child references throughout the graph
/// and clears stale caches on the node and all descendants.
pub fn write_current(dir: &Path, code: &str) -> Result<String, DtrError> {
    let old_hash = cwn_read(dir)?;
    let old_node = node_read(dir, &old_hash)?;

    // Write new blob (content-addressable, no-op if blob already exists)
    let new_blob_hash = blob_write(dir, code)?;

    // If the code hasn't changed, nothing to do
    if new_blob_hash == old_node.blob {
        return Ok(old_hash);
    }

    // Build new node with updated blob and cleared cache
    let new_node = Node {
        blob: new_blob_hash,
        cache: None,
        ..old_node.clone()
    };

    let new_hash = next_node_id(dir)?;
    let json = serde_json::to_string(&new_node)?;
    fs::write(dtr_path(dir).join("nodes").join(&new_hash), &json)?;

    // Update parent nodes: replace old_hash with new_hash in children lists
    for parent_hash in &new_node.parents {
        let mut parent = node_read(dir, parent_hash)?;
        if let Some(pos) = parent.children.iter().position(|c| c == &old_hash) {
            parent.children[pos] = new_hash.clone();
            node_write(dir, parent_hash, &parent)?;
        }
    }

    // Update child nodes: replace old_hash with new_hash in parents lists,
    // and recursively clear all descendant caches (they are now stale)
    for child_hash in &new_node.children {
        let mut child = node_read(dir, child_hash)?;
        if let Some(pos) = child.parents.iter().position(|p| p == &old_hash) {
            child.parents[pos] = new_hash.clone();
            node_write(dir, child_hash, &child)?;
        }
        clear_descendant_caches(dir, child_hash)?;
    }

    // Update CWN to point to the new node
    cwn_write(dir, &new_hash)?;

    Ok(new_hash)
}

/// Recursively clear all cached outputs from a node and its descendants.
fn clear_descendant_caches(dir: &Path, hash: &str) -> Result<(), DtrError> {
    let mut node = node_read(dir, hash)?;
    if node.cache.is_some() {
        node.cache = None;
        node_write(dir, hash, &node)?;
    }
    let children = node.children.clone();
    for child in &children {
        clear_descendant_caches(dir, child)?;
    }
    Ok(())
}

/// Delete the current working node.
/// If `recursive` is false, errors if the node has children.
/// If `recursive` is true, recursively deletes all descendants first.
/// Moves CWN to the first parent (or clears it if root node).
pub fn delete_current(dir: &Path, recursive: bool) -> Result<(), DtrError> {
    let hash = cwn_read(dir)?;
    let node = node_read(dir, &hash)?;

    // Check for children
    if !node.children.is_empty() {
        if recursive {
            // Recursively delete all children first (post-order)
            let children = node.children.clone();
            for child_hash in &children {
                // Move CWN to the child, delete it recursively
                cwn_write(dir, child_hash)?;
                delete_current(dir, true)?;
            }
            // Re-read self (children may have been updated, but our node is unchanged)
            // Actually our node file is still the same on disk
        } else {
            return Err(DtrError::InvalidState(
                format!(
                    "cannot delete node {} with {} children (use -r to force)",
                    hash,
                    node.children.len()
                )
            ));
        }
    }

    // Remove self from parent's children list
    for parent_hash in &node.parents {
        if let Ok(mut parent) = node_read(dir, parent_hash) {
            parent.children.retain(|c| c != &hash);
            node_write(dir, parent_hash, &parent)?;
        }
    }

    // Remove self from markers if any marker points to this node
    if let Ok(mut markers) = markers_read(dir) {
        markers.retain(|_, v| v.as_str() != Some(&hash));
        markers_write(dir, &markers)?;
    }

    // Remove cached output if any
    if let Some(ref cache_hash) = node.cache {
        let _ = fs::remove_file(dtr_path(dir).join("cache").join(cache_hash));
    }

    // Delete the node file
    let node_path = dtr_path(dir).join("nodes").join(&hash);
    let _ = fs::remove_file(&node_path);

    // Move CWN to first parent, or clear if root
    if let Some(parent) = node.parents.first() {
        cwn_write(dir, parent)?;
    } else {
        // Root node — clear CWN
        fs::write(dtr_path(dir).join("CWN"), "")?;
    }

    Ok(())
}

/// Compose the current working node into a single runnable R script.
/// Performs a DFS from the current node upward through parents, reverses
/// the order to produce a top-down pipe chain using R's native `|>` operator.
/// For merge nodes (multiple parents), non-primary parents are pre-computed
/// and assigned to variables named after their markers (falling back to
/// `parent_N`).
pub fn compose(dir: &Path) -> Result<String, DtrError> {
    let hash = cwn_read(dir)?;
    let composed = compose_node(dir, &hash)?;
    Ok(format!("{}{}", library_imports(dir)?, composed))
}

fn derive_parent_var_from(
    markers: Option<&serde_json::Map<String, serde_json::Value>>,
    hash: &str,
) -> String {
    marker_for_hash_in(markers, hash)
        .unwrap_or_else(|| format!("p_{}", &hash[..8.min(hash.len())]))
}

fn marker_for_hash_in(
    markers: Option<&serde_json::Map<String, serde_json::Value>>,
    hash: &str,
) -> Option<String> {
    markers?.iter().find_map(|(name, val)| {
        if val.as_str() == Some(hash) {
            Some(name.clone())
        } else {
            None
        }
    })
}

/// Shared implementation for recursive R script composition.
/// `recurse` controls which function to call on parent nodes,
/// enabling both the normal compose path and the cache-aware variant.
fn compose_impl(
    dir: &Path,
    hash: &str,
    recurse: fn(&Path, &str) -> Result<String, DtrError>,
) -> Result<String, DtrError> {
    let node = node_read(dir, hash)?;
    let code = fs::read_to_string(dtr_path(dir).join("blobs").join(&node.blob))?;
    let markers = markers_read(dir).ok();

    match node.parents.len() {
        0 => Ok(code),
        1 => {
            let parent_script = recurse(dir, &node.parents[0])?;
            Ok(format!("{} |>\n  {}", parent_script, code))
        }
        _ => {
            let mut script = String::new();
            for (i, parent_hash) in node.parents.iter().enumerate().skip(1) {
                let var_name = node
                    .parent_vars
                    .get(i)
                    .filter(|v| !v.is_empty())
                    .cloned()
                    .unwrap_or_else(|| {
                        marker_for_hash_in(markers.as_ref(), parent_hash)
                            .unwrap_or_else(|| format!("parent_{}", i + 1))
                    });
                let branch = recurse(dir, parent_hash)?;
                script.push_str(&format!("{} <- {}\n\n", var_name, branch));
            }
            let main_branch = recurse(dir, &node.parents[0])?;
            script.push_str(&format!("{} |>\n  {}", main_branch, code));
            Ok(script)
        }
    }
}

/// Recursively build the R script for a node and all its ancestors.
fn compose_node(dir: &Path, hash: &str) -> Result<String, DtrError> {
    compose_impl(dir, hash, compose_node)
}

/// Like `compose_node`, but stops recursion at any ancestor that has a
/// cached output on disk — returns `readRDS('path')` instead of composing
/// further up that branch.
fn compose_node_cached(dir: &Path, hash: &str) -> Result<String, DtrError> {
    let node = node_read(dir, hash)?;
    if let Some(ref cache_hash) = node.cache {
        let cache_path = dtr_path(dir).join("cache").join(cache_hash);
        if cache_path.exists() {
            return Ok(format!("readRDS('{}')", cache_path.display()));
        }
    }
    compose_impl(dir, hash, compose_node_cached)
}

/// Run the current working node.
/// If `force` is true, recomputes all ancestors from scratch (`dtr run -r`).
/// Otherwise, stops recursing at cached ancestors and uses their stored
/// RDS output (`dtr run`). Caches the result as RDS in `.dtr/cache/<hash>`.
/// Returns the text output.
pub fn run(dir: &Path, force: bool) -> Result<String, DtrError> {
    let hash = cwn_read(dir)?;

    // Build the R script
    let libs = library_imports(dir)?;
    let composed = if force {
        compose_node(dir, &hash)?
    } else {
        compose_node_cached(dir, &hash)?
    };

    // Wrap with result capture and RDS save (libs go before result <-)
    let tmp_rds = dtr_path(dir).join("cache").join(".tmp_result.rds");
    let script = wrap_r_script(&libs, &composed, &tmp_rds);

    // Execute via Rscript
    let output = execute_r_script(&script)?;

    // Cache the RDS result if it was created
    if let Ok(rds_bytes) = fs::read(&tmp_rds) {
        let cache_hash = hash_bytes(&rds_bytes);
        let final_path = dtr_path(dir).join("cache").join(&cache_hash);
        let _ = fs::rename(&tmp_rds, &final_path);

        let mut node = node_read(dir, &hash)?;
        node.cache = Some(cache_hash);
        node_write(dir, &hash, &node)?;
    }
    let _ = fs::remove_file(&tmp_rds);

    Ok(output)
}

/// Wrap a composed R pipe chain so that it captures the result
/// via `print()` and saves it as RDS for future caching.
///
/// `library_imports` (e.g. "library(dplyr)\nlibrary(ggplot2)\n\n")
/// are placed BEFORE the `result <-` assignment so that the pipe
/// chain — not a `library()` call — is captured as the result.
///
/// The composed chain is wrapped in braces `{{ ... }}` so that when
/// merge nodes emit multiple statements (side-parent assignments + main
/// pipe), the *last* expression's value is captured as `result`.
fn wrap_r_script(library_imports: &str, composed_chain: &str, rds_path: &Path) -> String {
    format!(
        "{library_imports}result <- {{\n{composed_chain}\n}}\nprint(result)\nsaveRDS(result, '{}')\n",
        rds_path.display()
    )
}

/// Execute an R script via `Rscript` and return its stdout.
fn execute_r_script(script: &str) -> Result<String, DtrError> {
    // Use a random suffix to avoid collisions between concurrent calls
    let tmp = std::env::temp_dir().join(format!(
        "dtr_run_{}_{:x}.R",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_nanos()
    ));
    fs::write(&tmp, script)?;

    let output = std::process::Command::new("Rscript")
        .arg(&tmp)
        .output()
        .map_err(|e| DtrError::RExecError(format!("failed to launch R: {e}")))?;

    let _ = fs::remove_file(&tmp);

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(DtrError::RExecError(stderr.to_string()));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Cache the output of the current working node as RDS.
/// Runs the composed R script, saves the result as RDS, hashes it,
/// stores it in `.dtr/cache/<sha1>`, and sets the node's `cache` field.
/// Returns the cache hash.
pub fn cache(dir: &Path) -> Result<String, DtrError> {
    let hash = cwn_read(dir)?;

    let libs = library_imports(dir)?;
    let composed = compose_node(dir, &hash)?;
    let tmp_rds = dtr_path(dir).join("cache").join(".tmp_result.rds");
    let script = wrap_r_script(&libs, &composed, &tmp_rds);

    // Run R to produce the RDS file
    let _output = execute_r_script(&script)?;

    // Hash and store the RDS
    let rds_bytes = fs::read(&tmp_rds)
        .map_err(|_| DtrError::InvalidState("R produced no output".to_string()))?;
    let cache_hash = hash_bytes(&rds_bytes);
    let final_path = dtr_path(dir).join("cache").join(&cache_hash);
    if !final_path.exists() {
        fs::rename(&tmp_rds, &final_path)?;
    } else {
        let _ = fs::remove_file(&tmp_rds);
    }

    let mut node = node_read(dir, &hash)?;
    node.cache = Some(cache_hash.clone());
    node_write(dir, &hash, &node)?;

    Ok(cache_hash)
}

/// Result of previewing a cached RDS object.
pub enum PreviewOutput {
    /// Plain text output (tibble preview, model summary, etc.)
    Text(String),
    /// Raw PNG bytes (ggplot objects rendered to raster)
    Png(Vec<u8>),
}

/// Preview the cached RDS output of a node.
///
/// If `target` is None, previews the current working node.
/// Otherwise resolves a node ID or marker name.
///
/// Behavior depends on the R object type:
/// - tibble / data.frame → text preview (head 20 rows)
/// - ggplot → raw PNG bytes
/// - model (lm, glm, etc.) → summary text
/// - other → print() output
pub fn preview(dir: &Path, target: Option<&str>) -> Result<PreviewOutput, DtrError> {
    let hash = match target {
        Some(t) => resolve_ref(dir, t)?,
        None => cwn_read(dir)?,
    };

    let node = node_read(dir, &hash)?;
    let cache_hash = node.cache.as_ref().ok_or_else(|| {
        DtrError::InvalidState(
            "node has no cached output (run dtr run or dtr cache first)".to_string(),
        )
    })?;

    let cache_path = dtr_path(dir).join("cache").join(cache_hash);
    if !cache_path.exists() {
        return Err(DtrError::InvalidState(
            "cache file not found on disk".to_string(),
        ));
    }

    let libs = library_imports(dir)?;
    let png_tmp = dtr_path(dir).join("preview_tmp.png");
    let script = build_preview_r_script(&libs, &cache_path, &png_tmp);

    let output = execute_r_script(&script)?;

    // If R produced a PNG, read it and return the bytes
    if output.trim() == "__DTR_PNG__" && png_tmp.exists() {
        let bytes = fs::read(&png_tmp)?;
        let _ = fs::remove_file(&png_tmp);
        return Ok(PreviewOutput::Png(bytes));
    }

    Ok(PreviewOutput::Text(output))
}

/// Build an R script that loads a cached RDS object and produces an
/// appropriate preview depending on the object type.
///
/// For ggplot objects, saves a PNG to `png_path` and prints `__DTR_PNG__`
/// to stdout as a signal. For everything else, prints text to stdout.
fn build_preview_r_script(library_imports: &str, cache_path: &Path, png_path: &Path) -> String {
    let cache_path_escaped = cache_path.display().to_string().replace('\\', "\\\\").replace('\'', "\\'");
    let png_path_escaped = png_path.display().to_string().replace('\\', "\\\\").replace('\'', "\\'");
    format!(
        r#"{library_imports}obj <- readRDS('{cache_path_escaped}')
if (inherits(obj, 'ggplot')) {{
  suppressMessages(ggplot2::ggsave('{png_path_escaped}', plot = obj, width = 8, height = 6, dpi = 100))
  cat('__DTR_PNG__\n')
}} else if (inherits(obj, 'data.frame')) {{
  print(head(obj, 20))
}} else if (inherits(obj, 'lm') || inherits(obj, 'glm')) {{
  print(summary(obj))
}} else {{
  print(obj)
}}
"#
    )
}

/// Clear cached output of the current node.
///
/// Flags:
/// - `recurse_children`: clear current node + all descendants
/// - `recurse_parents`: clear current node + all ancestors
/// - `all`: ignore CWN; clear all cache files and all node cache fields
///
/// When `all` is true, recurse_children/recurse_parents are ignored.
/// When neither recurse flag is set, only the current node's cache is cleared.
pub fn clear_cache(
    dir: &Path,
    recurse_children: bool,
    recurse_parents: bool,
    all: bool,
) -> Result<(), DtrError> {
    if all {
        return clear_all_caches(dir);
    }

    let hash = cwn_read(dir)?;

    // Verify node exists
    node_read(dir, &hash)?;

    // Clear current node's cache
    clear_node_cache(dir, &hash)?;

    if recurse_children {
        clear_descendant_caches(dir, &hash)?;
    }

    if recurse_parents {
        clear_ancestor_caches_recursive(dir, &hash)?;
    }

    Ok(())
}

/// Clear the cache field of a single node (keeps cache file on disk).
fn clear_node_cache(dir: &Path, hash: &str) -> Result<(), DtrError> {
    let mut node = node_read(dir, hash)?;
    if node.cache.is_some() {
        node.cache = None;
        node_write(dir, hash, &node)?;
    }
    Ok(())
}

/// Clear all caches: remove all files in cache/ and clear cache fields
/// on every node in nodes/.
fn clear_all_caches(dir: &Path) -> Result<(), DtrError> {
    // Clear all node cache fields
    let nodes_dir = dtr_path(dir).join("nodes");
    if nodes_dir.exists() {
        for entry in fs::read_dir(&nodes_dir)? {
            let entry = entry?;
            let hash = entry.file_name().to_string_lossy().to_string();
            clear_node_cache(dir, &hash)?;
        }
    }

    // Remove all files from cache/ directory (keep the directory itself)
    let cache_dir = dtr_path(dir).join("cache");
    if cache_dir.exists() {
        for entry in fs::read_dir(&cache_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                let _ = fs::remove_file(&path);
            }
        }
    }

    Ok(())
}

/// Clear caches recursively up through parents (ancestors).
fn clear_ancestor_caches_recursive(dir: &Path, hash: &str) -> Result<(), DtrError> {
    let node = node_read(dir, hash)?;
    for parent in &node.parents {
        clear_node_cache(dir, parent)?;
        clear_ancestor_caches_recursive(dir, parent)?;
    }
    Ok(())
}

/// Serialize part or all of the DAG as JSON.
///
/// By default, returns all nodes with their IDs, types, marker names,
/// parents, children, blob hash, and cache hash.
///
/// - `recurse_children`: only current node + descendants
/// - `recurse_parents`: only current node + ancestors
/// - neither: all nodes in the project
pub fn map(dir: &Path, recurse_children: bool, recurse_parents: bool) -> Result<String, DtrError> {
    let markers = markers_read(dir).ok();
    let reverse_markers = build_reverse_marker_map(markers.as_ref());

    let selected_ids: std::collections::BTreeSet<String> = if recurse_children || recurse_parents {
        let cwn = cwn_read(dir)?;
        // Verify CWN exists
        node_read(dir, &cwn)?;
        let mut ids = std::collections::BTreeSet::new();
        ids.insert(cwn.clone());
        if recurse_children {
            collect_descendant_ids(dir, &cwn, &mut ids)?;
        }
        if recurse_parents {
            collect_ancestor_ids(dir, &cwn, &mut ids)?;
        }
        ids
    } else {
        // All nodes
        let nodes_dir = dtr_path(dir).join("nodes");
        let mut ids = std::collections::BTreeSet::new();
        if nodes_dir.exists() {
            for entry in fs::read_dir(&nodes_dir)? {
                let entry = entry?;
                ids.insert(entry.file_name().to_string_lossy().to_string());
            }
        }
        ids
    };

    // Build the JSON object
    let nodes_map: serde_json::Map<String, serde_json::Value> = selected_ids
        .iter()
        .filter_map(|id| {
            let node = node_read(dir, id).ok()?;
            let marker = reverse_markers.get(id).cloned();
            let entry = serde_json::json!({
                "node_type": node.node_type,
                "parents": node.parents,
                "children": node.children,
                "blob": node.blob,
                "cache": node.cache,
                "marker": marker,
            });
            Some((id.clone(), entry))
        })
        .collect();

    let output = serde_json::json!({"nodes": nodes_map});
    Ok(serde_json::to_string_pretty(&output)?)
}

/// Build a reverse lookup: node ID → marker name.
fn build_reverse_marker_map(
    markers: Option<&serde_json::Map<String, serde_json::Value>>,
) -> std::collections::HashMap<String, String> {
    let mut map = std::collections::HashMap::new();
    if let Some(m) = markers {
        for (name, val) in m {
            if let Some(hash) = val.as_str() {
                map.insert(hash.to_string(), name.clone());
            }
        }
    }
    map
}

/// Collect all descendant node IDs (children, grandchildren, etc.) into `ids`.
fn collect_descendant_ids(
    dir: &Path,
    hash: &str,
    ids: &mut std::collections::BTreeSet<String>,
) -> Result<(), DtrError> {
    let node = node_read(dir, hash)?;
    for child in &node.children {
        if ids.insert(child.clone()) {
            collect_descendant_ids(dir, child, ids)?;
        }
    }
    Ok(())
}

/// Collect all ancestor node IDs (parents, grandparents, etc.) into `ids`.
fn collect_ancestor_ids(
    dir: &Path,
    hash: &str,
    ids: &mut std::collections::BTreeSet<String>,
) -> Result<(), DtrError> {
    let node = node_read(dir, hash)?;
    for parent in &node.parents {
        if ids.insert(parent.clone()) {
            collect_ancestor_ids(dir, parent, ids)?;
        }
    }
    Ok(())
}

/// Give the current working node a marker (nickname).
/// Returns the node hash the marker points to.
pub fn add_marker(dir: &Path, name: &str) -> Result<String, DtrError> {
    let node_hash = cwn_read(dir)?;
    // Verify the node exists
    node_read(dir, &node_hash)?;

    let mut markers = markers_read(dir)?;
    markers.insert(name.to_string(), serde_json::Value::String(node_hash.clone()));
    markers_write(dir, &markers)?;

    Ok(node_hash)
}

/// Add an R package to the project's library list.
/// The package will be imported via `library()` at the top of every
/// composed or run R script.
pub fn add_lib(dir: &Path, package: &str) -> Result<(), DtrError> {
    let mut packages = packages_read(dir)?;
    if !packages.contains(&package.to_string()) {
        packages.push(package.to_string());
        packages_write(dir, &packages)?;
    }
    Ok(())
}

/// Read the markers map.
pub fn read_markers(dir: &Path) -> Result<serde_json::Map<String, serde_json::Value>, DtrError> {
    markers_read(dir)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("dtr_test_{}_{}", std::process::id(), name));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    fn setup(name: &str) -> std::path::PathBuf {
        let dir = temp_dir(name);
        init(&dir).expect("init should succeed");
        dir
    }

    fn assert_dtr_structure(dir: &Path) {
        let dtr = dir.join(".dtr");
        assert!(dtr.exists(), ".dtr directory should exist");
        assert!(dtr.is_dir(), ".dtr should be a directory");

        assert!(dtr.join("blobs").exists(), "blobs/ should exist");
        assert!(dtr.join("blobs").is_dir(), "blobs/ should be a directory");

        assert!(dtr.join("nodes").exists(), "nodes/ should exist");
        assert!(dtr.join("nodes").is_dir(), "nodes/ should be a directory");

        assert!(dtr.join("cache").exists(), "cache/ should exist");
        assert!(dtr.join("cache").is_dir(), "cache/ should be a directory");

        let cwn_path = dtr.join("CWN");
        assert!(cwn_path.exists(), "CWN should exist");
        let cwn_content = fs::read_to_string(cwn_path).expect("CWN should be readable");
        assert_eq!(cwn_content, "", "CWN should be empty on init");

        let next_id_path = dtr.join("next_id");
        assert!(next_id_path.exists(), "next_id should exist");
        let next_id_content = fs::read_to_string(next_id_path).expect("next_id readable");
        assert_eq!(next_id_content, "0", "next_id should start at 0");

        let markers_path = dtr.join("markers");
        assert!(markers_path.exists(), "markers should exist");
        let markers_content = fs::read_to_string(markers_path).expect("markers should be readable");
        assert_eq!(markers_content, "{}\n", "markers should contain empty JSON object");

        let packages_path = dtr.join("packages");
        assert!(packages_path.exists(), "packages should exist");
        let packages_content = fs::read_to_string(packages_path).expect("packages readable");
        assert_eq!(packages_content, "[]\n", "packages should be empty array");
    }

    // -----------------------------------------------------------------------
    // Init tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_init_creates_dtr_structure() {
        let dir = temp_dir("creates_structure");
        init(&dir).expect("init should succeed");
        assert_dtr_structure(&dir);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_init_is_idempotent() {
        let dir = temp_dir("idempotent");
        init(&dir).expect("first init should succeed");
        init(&dir).expect("second init should also succeed (idempotent)");
        assert_dtr_structure(&dir);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_init_on_nested_path() {
        let dir = temp_dir("nested").join("nested").join("project");
        init(&dir).expect("init on nested path should succeed");
        assert_dtr_structure(&dir);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_init_markers_is_valid_json() {
        let dir = temp_dir("markers_json");
        init(&dir).expect("init should succeed");
        let markers_path = dir.join(".dtr").join("markers");
        let content = fs::read_to_string(markers_path).expect("read markers");
        let parsed: serde_json::Value =
            serde_json::from_str(&content).expect("markers should be valid JSON");
        assert_eq!(parsed, serde_json::json!({}), "markers should be empty object");
        let _ = fs::remove_dir_all(&dir);
    }

    // -----------------------------------------------------------------------
    // Add tests: input node (first node, no CWN)
    // -----------------------------------------------------------------------

    #[test]
    fn test_add_first_input_creates_node() {
        let dir = setup("first_input_creates_node");

        let code = "read_csv('data.csv')";
        let hash = add_input(&dir, code, None).expect("add input should succeed");

        // Blob should exist with the code
        let blob = read_blob(&dir, &hash_string(code)).expect("read blob");
        assert_eq!(blob, code, "blob content should match code");

        // Node should exist
        let node = read_node(&dir, &hash).expect("read node");
        assert_eq!(node.node_type, "input");
        assert!(node.parents.is_empty(), "first input should have no parents");
        assert!(node.children.is_empty(), "new node should have no children");
        assert_eq!(node.blob, hash_string(code));
        assert!(node.cache.is_none());

        // CWN should point to this node
        let cwn = read_cwn(&dir).expect("read cwn");
        assert_eq!(cwn, hash, "CWN should point to new node");
    }

    #[test]
    fn test_add_input_with_custom_marker() {
        let dir = setup("input_custom_marker");

        let code = "read_csv('data.csv')";
        let hash = add_input(&dir, code, Some("mydata")).expect("add input with marker");

        let markers = read_markers(&dir).expect("read markers");
        assert_eq!(markers.get("mydata"), Some(&serde_json::Value::String(hash.clone())));
    }

    #[test]
    fn test_add_input_auto_marker() {
        let dir = setup("input_auto_marker");

        let code1 = "read_csv('a.csv')";
        let hash1 = add_input(&dir, code1, None).expect("first input");

        let markers = read_markers(&dir).expect("read markers");
        assert_eq!(markers.get("input_1"), Some(&serde_json::Value::String(hash1.clone())));

        let code2 = "read_csv('b.csv')";
        let hash2 = add_input(&dir, code2, None).expect("second input");

        let markers = read_markers(&dir).expect("read markers");
        assert_eq!(markers.get("input_1"), Some(&serde_json::Value::String(hash1)));
        assert_eq!(markers.get("input_2"), Some(&serde_json::Value::String(hash2)));
    }

    #[test]
    fn test_add_input_multiple_no_conflict() {
        let dir = setup("input_multiple");

        let h1 = add_input(&dir, "read_csv('x.csv')", Some("source")).expect("first input");
        let h2 = add_input(&dir, "read_csv('y.csv')", None).expect("second input");
        let h3 = add_input(&dir, "read_csv('z.csv')", None).expect("third input");

        // When "source" is used, auto-marker should skip conflict
        let markers = read_markers(&dir).expect("read markers");
        assert_eq!(markers.get("source"), Some(&serde_json::Value::String(h1)));
        assert_eq!(markers.get("input_1"), Some(&serde_json::Value::String(h2)));
        assert_eq!(markers.get("input_2"), Some(&serde_json::Value::String(h3)));
    }

    // -----------------------------------------------------------------------
    // Add tests: chain (input -> chart -> model)
    // -----------------------------------------------------------------------

    #[test]
    fn test_add_chart_as_child() {
        let dir = setup("add_chart_child");

        let input_code = "read_csv('data.csv')";
        let input_hash = add_input(&dir, input_code, None).expect("add input");

        let chart_code = "ggplot(aes(x, y)) + geom_point()";
        let chart_hash = add(&dir, "chart", chart_code, None).expect("add chart");

        // Chart node
        let chart = read_node(&dir, &chart_hash).expect("read chart");
        assert_eq!(chart.node_type, "chart");
        assert_eq!(chart.parents, vec![input_hash.clone()], "chart parent should be input");
        assert_eq!(chart.blob, hash_string(chart_code));

        // Input node should have chart as child
        let input = read_node(&dir, &input_hash).expect("read input");
        assert_eq!(input.children, vec![chart_hash.clone()], "input should have chart as child");

        // CWN should point to chart
        let cwn = read_cwn(&dir).expect("read cwn");
        assert_eq!(cwn, chart_hash);
    }

    #[test]
    fn test_add_model_on_chart() {
        let dir = setup("add_model_on_chart");

        let _input = add_input(&dir, "read_csv('d.csv')", None).expect("add input");
        let chart = add(&dir, "chart", "ggplot(aes(x, y)) + geom_point()", None).expect("add chart");
        let model = add(&dir, "model", "glm(y ~ x, family = binomial)", None).expect("add model");

        let model_node = read_node(&dir, &model).expect("read model");
        assert_eq!(model_node.node_type, "model");
        assert_eq!(model_node.parents, vec![chart.clone()], "model parent should be chart");

        // Chart should have model as child
        let chart_node = read_node(&dir, &chart).expect("read chart");
        assert_eq!(chart_node.children, vec![model], "chart should have model as child");
        let _ = chart_node;
        let _ = model;
    }

    #[test]
    fn test_add_updates_cwn() {
        let dir = setup("add_updates_cwn");

        let h1 = add_input(&dir, "read_csv('a.csv')", None).expect("input");
        assert_eq!(read_cwn(&dir).unwrap(), h1);

        let h2 = add(&dir, "chart", "ggplot()", None).expect("chart");
        assert_eq!(read_cwn(&dir).unwrap(), h2);

        let h3 = add(&dir, "model", "glm()", None).expect("model");
        assert_eq!(read_cwn(&dir).unwrap(), h3);
    }

    // -----------------------------------------------------------------------
    // Add tests: merge
    // -----------------------------------------------------------------------

    #[test]
    fn test_add_merge_with_two_parents() {
        let dir = setup("merge_two_parents");

        let left = add_input(&dir, "read_csv('left.csv')", Some("left")).expect("left input");
        let right = add_input(&dir, "read_csv('right.csv')", Some("right")).expect("right input");

        // Manually set CWN to left so we can verify merge doesn't use CWN
        // (merge specifies parents explicitly)
        cwn_write(&dir, &left).expect("write cwn");

        let merge_code = "inner_join(right, by = 'id')";
        let merge_hash = add_merge(&dir, merge_code, &[&left, &right], None).expect("add merge");

        let merge_node = read_node(&dir, &merge_hash).expect("read merge");
        assert_eq!(merge_node.node_type, "merge");
        assert_eq!(merge_node.parents, vec![left.clone(), right.clone()]);

        // Both parents should list merge as child
        let left_node = read_node(&dir, &left).expect("read left");
        assert!(left_node.children.contains(&merge_hash), "left should have merge as child");

        let right_node = read_node(&dir, &right).expect("read right");
        assert!(right_node.children.contains(&merge_hash), "right should have merge as child");

        // CWN should point to merge
        assert_eq!(read_cwn(&dir).unwrap(), merge_hash);
    }

    #[test]
    fn test_add_merge_by_marker_name() {
        let dir = setup("merge_by_marker");

        let left = add_input(&dir, "read_csv('left.csv')", Some("left")).expect("left");
        let right = add_input(&dir, "read_csv('right.csv')", Some("right")).expect("right");

        // Use marker names instead of hashes
        let merge_hash = add_merge(&dir, "inner_join(right, by = 'id')", &["left", "right"], None)
            .expect("add merge by markers");

        let merge_node = read_node(&dir, &merge_hash).expect("read merge");
        assert_eq!(merge_node.parents, vec![left, right], "should resolve markers");
        assert_eq!(merge_node.parent_vars, vec!["", "right"], "should derive vars from markers");
    }

    #[test]
    fn test_add_merge_mixed_hash_and_marker() {
        let dir = setup("merge_mixed");

        let left = add_input(&dir, "read_csv('left.csv')", None).expect("left");
        let right = add_input(&dir, "read_csv('right.csv')", Some("right")).expect("right");

        // Mix: left by hash, right by marker name
        let merge_hash =
            add_merge(&dir, "inner_join(right, by = 'id')", &[&left, "right"], None).expect("add merge");

        let merge_node = read_node(&dir, &merge_hash).expect("read merge");
        assert_eq!(merge_node.parents, vec![left, right]);
    }

    #[test]
    fn test_add_merge_bad_marker() {
        let dir = setup("merge_bad_marker");

        let _left = add_input(&dir, "read_csv('left.csv')", Some("left")).expect("left");

        let result = add_merge(&dir, "inner_join()", &["left", "nonexistent"], None);
        assert!(result.is_err(), "bad marker should error");
        match result {
            Err(DtrError::NodeNotFound(name)) => assert!(name.contains("nonexistent")),
            _ => panic!("expected NodeNotFound error"),
        }
    }

    #[test]
    fn test_add_merge_errors_with_one_parent() {
        let dir = setup("merge_one_parent_error");

        let input = add_input(&dir, "read_csv('d.csv')", None).expect("add input");

        let result = add_merge(&dir, "inner_join()", &[&input], None);
        assert!(result.is_err(), "merge with one parent should error");
        match result {
            Err(DtrError::InvalidState(msg)) => assert!(msg.contains("at least 2 parents")),
            _ => panic!("expected InvalidState error"),
        }
    }

    #[test]
    fn test_add_merge_errors_with_nonexistent_parent() {
        let dir = setup("merge_bad_parent");

        let result = add_merge(&dir, "inner_join()", &["deadbeef", "cafebabe"], None);
        assert!(result.is_err(), "merge with bad parents should error");
        match result {
            Err(DtrError::NodeNotFound(_)) => {} // expected
            _ => panic!("expected NodeNotFound error"),
        }
    }

    // -----------------------------------------------------------------------
    // Add tests: error cases
    // -----------------------------------------------------------------------

    #[test]
    fn test_add_errors_without_cwn() {
        let dir = setup("add_no_cwn");

        // CWN is empty after init — adding a chart (non-input) should fail
        let result = add(&dir, "chart", "ggplot()", None);
        assert!(result.is_err(), "add with no CWN should error");
        match result {
            Err(DtrError::NoCurrentNode) => {} // expected
            _ => panic!("expected NoCurrentNode error"),
        }
    }

    #[test]
    fn test_add_input_works_without_cwn() {
        let dir = setup("add_input_no_cwn");

        // Even with empty CWN, add_input should work (it's a root node)
        let hash = add_input(&dir, "read_csv('data.csv')", None);
        assert!(hash.is_ok(), "add_input should work without CWN");
    }

    #[test]
    fn test_add_invalid_node_type() {
        let dir = setup("invalid_type");

        let result = add_input(&dir, "read_csv('d.csv')", None);
        assert!(result.is_ok(), "input is valid");

        let result = add(&dir, "invalid_type", "some code", None);
        assert!(result.is_err(), "invalid node type should error");
        match result {
            Err(DtrError::InvalidNodeType(t)) => assert_eq!(t, "invalid_type"),
            _ => panic!("expected InvalidNodeType error"),
        }
    }

    #[test]
    fn test_add_process_as_valid_type() {
        let dir = setup("add_process_valid");

        let input = add_input(&dir, "read_csv('data.csv')", None).expect("add input");
        let result = add(&dir, "process", "mutate(z = x + y)", None);
        assert!(result.is_ok(), "process should be a valid node type");

        let process_hash = result.unwrap();
        let process = read_node(&dir, &process_hash).expect("read process");
        assert_eq!(process.node_type, "process");
        assert_eq!(process.parents, vec![input]);
    }

    #[test]
    fn test_add_process_chain() {
        let dir = setup("add_process_chain");

        let _input = add_input(&dir, "read_csv('data.csv')", None).expect("add input");
        let process = add(&dir, "process", "filter(x > 0)", None).expect("add filter");
        let chart = add(&dir, "chart", "ggplot(aes(x, y)) + geom_point()", None).expect("add chart");

        let process_node = read_node(&dir, &process).expect("read process");
        assert_eq!(process_node.node_type, "process");
        assert!(!process_node.children.is_empty(), "process should have chart as child");

        let chart_node = read_node(&dir, &chart).expect("read chart");
        assert_eq!(chart_node.node_type, "chart");
        assert_eq!(chart_node.parents, vec![process], "chart parent should be process");
    }

    #[test]
    fn test_add_process_with_marker() {
        let dir = setup("add_process_marker");

        let input = add_input(&dir, "read_csv('data.csv')", None).expect("input");
        let hash = add(&dir, "process", "filter(x > 0)", Some("filtered")).expect("process");

        let markers = read_markers(&dir).expect("read markers");
        assert_eq!(
            markers.get("filtered"),
            Some(&serde_json::Value::String(hash.clone())),
            "marker should point to process node"
        );
    }

    #[test]
    fn test_add_chart_with_marker() {
        let dir = setup("add_chart_marker");

        let _input = add_input(&dir, "read_csv('data.csv')", None).expect("input");
        let hash = add(&dir, "chart", "ggplot(aes(x,y)) + geom_point()", Some("myplot")).expect("chart");

        let markers = read_markers(&dir).expect("read markers");
        assert_eq!(
            markers.get("myplot"),
            Some(&serde_json::Value::String(hash)),
            "marker should point to chart node"
        );
    }

    #[test]
    fn test_add_model_with_marker() {
        let dir = setup("add_model_marker");

        let _input = add_input(&dir, "read_csv('data.csv')", None).expect("input");
        let hash = add(&dir, "model", "glm(y ~ x)", Some("mymodel")).expect("model");

        let markers = read_markers(&dir).expect("read markers");
        assert_eq!(
            markers.get("mymodel"),
            Some(&serde_json::Value::String(hash)),
            "marker should point to model node"
        );
    }

    #[test]
    fn test_add_merge_with_marker() {
        let dir = setup("add_merge_marker");

        let left = add_input(&dir, "read_csv('left.csv')", Some("left")).expect("left");
        let right = add_input(&dir, "read_csv('right.csv')", Some("right")).expect("right");
        let hash = add_merge(&dir, "inner_join(right, by = 'id')", &[&left, &right], Some("joined")).expect("merge");

        let markers = read_markers(&dir).expect("read markers");
        assert_eq!(
            markers.get("joined"),
            Some(&serde_json::Value::String(hash)),
            "marker should point to merge node"
        );
    }

    #[test]
    fn test_add_process_without_marker_no_side_effect() {
        let dir = setup("add_process_no_marker");

        let _input = add_input(&dir, "read_csv('data.csv')", None).expect("input");
        add(&dir, "process", "filter(x > 0)", None).expect("process");

        let markers = read_markers(&dir).expect("read markers");
        // Only the auto-generated input_1 marker should exist
        assert_eq!(markers.len(), 1, "no extra markers created");
        assert!(markers.contains_key("input_1"), "only input_1 marker");
    }

    #[test]
    fn test_add_errors_on_nonexistent_parent() {
        let dir = setup("add_bad_parent");

        // Manually write a bad CWN
        cwn_write(&dir, "nonexistent_hash").expect("write bad cwn");

        let result = add(&dir, "chart", "ggplot()", None);
        assert!(result.is_err(), "add with bad parent should error");
        match result {
            Err(DtrError::NodeNotFound(_)) => {} // expected
            _ => panic!("expected NodeNotFound error"),
        }
    }

    // -----------------------------------------------------------------------
    // Goto tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_goto_by_hash() {
        let dir = setup("goto_by_hash");

        let a = add_input(&dir, "read_csv('a.csv')", None).expect("a");
        let b = add_input(&dir, "read_csv('b.csv')", None).expect("b");

        // Move CWN to a by hash
        goto(&dir, &a).expect("goto a");
        assert_eq!(read_cwn(&dir).unwrap(), a, "CWN should be at a");

        // Move CWN to b by hash
        goto(&dir, &b).expect("goto b");
        assert_eq!(read_cwn(&dir).unwrap(), b, "CWN should be at b");
    }

    #[test]
    fn test_goto_by_marker() {
        let dir = setup("goto_by_marker");

        let hash = add_input(&dir, "read_csv('data.csv')", Some("mydata")).expect("input");
        let _other = add_input(&dir, "read_csv('other.csv')", None).expect("other");

        // CWN is at other. Goto mydata by marker.
        goto(&dir, "mydata").expect("goto mydata");
        assert_eq!(read_cwn(&dir).unwrap(), hash, "CWN should be at mydata");
    }

    #[test]
    fn test_goto_parent_shortcut() {
        let dir = setup("goto_parent");

        let input = add_input(&dir, "read_csv('data.csv')", None).expect("input");
        let _chart = add(&dir, "chart", "ggplot()", None).expect("chart");

        // CWN is at chart. Goto parent should move to input.
        goto(&dir, "..").expect("goto parent");
        assert_eq!(read_cwn(&dir).unwrap(), input, "CWN should be at parent");
    }

    #[test]
    fn test_goto_child_shortcut() {
        let dir = setup("goto_child");

        let input = add_input(&dir, "read_csv('data.csv')", None).expect("input");
        let chart = add(&dir, "chart", "ggplot()", None).expect("chart");

        // Move back to input, then goto child
        goto(&dir, &input).expect("goto input");
        goto(&dir, ".").expect("goto child");
        assert_eq!(read_cwn(&dir).unwrap(), chart, "CWN should be at child");
    }

    #[test]
    fn test_goto_parent_multiple_parents_error() {
        let dir = setup("goto_multi_parent");

        let left = add_input(&dir, "read_csv('left.csv')", None).expect("left");
        let right = add_input(&dir, "read_csv('right.csv')", None).expect("right");
        add_merge(&dir, "inner_join()", &[&left, &right], None).expect("merge");

        // CWN is at merge with two parents. Goto .. should error.
        let result = goto(&dir, "..");
        assert!(result.is_err(), "goto parent with multiple parents should error");
        match result {
            Err(DtrError::InvalidState(msg)) => assert!(msg.contains("multiple parents")),
            _ => panic!("expected InvalidState error"),
        }

        // CWN should be unchanged
        let cwn = read_cwn(&dir).expect("read cwn");
        let merge_node = read_node(&dir, &cwn).expect("read merge");
        assert_eq!(merge_node.node_type, "merge", "state should be unchanged");
    }

    #[test]
    fn test_goto_child_multiple_children_error() {
        let dir = setup("goto_multi_child");

        let input = add_input(&dir, "read_csv('data.csv')", None).expect("input");
        let _chart = add(&dir, "chart", "ggplot()", None).expect("chart");

        // Go back to input, then branch again
        goto(&dir, &input).expect("goto input");
        let _model = add(&dir, "model", "glm(y ~ x)", None).expect("model");

        // CWN is at model. Goto back to input which now has 2 children.
        goto(&dir, &input).expect("goto input");

        // Goto . should error since input has 2 children
        let result = goto(&dir, ".");
        assert!(result.is_err(), "goto child with multiple children should error");
        match result {
            Err(DtrError::InvalidState(msg)) => assert!(msg.contains("multiple children")),
            _ => panic!("expected InvalidState error"),
        }

        // CWN should be unchanged
        assert_eq!(read_cwn(&dir).unwrap(), input, "state should be unchanged");
    }

    #[test]
    fn test_goto_parent_of_root_error() {
        let dir = setup("goto_parent_root");

        add_input(&dir, "read_csv('data.csv')", None).expect("input");

        // CWN is at input (root, no parents)
        let result = goto(&dir, "..");
        assert!(result.is_err(), "goto parent of root should error");
        match result {
            Err(DtrError::InvalidState(msg)) => assert!(msg.contains("no parent")),
            _ => panic!("expected InvalidState error"),
        }
    }

    #[test]
    fn test_goto_child_of_leaf_error() {
        let dir = setup("goto_child_leaf");

        let _input = add_input(&dir, "read_csv('data.csv')", None).expect("input");
        let _chart = add(&dir, "chart", "ggplot()", None).expect("chart");

        // CWN is at chart (leaf, no children)
        let result = goto(&dir, ".");
        assert!(result.is_err(), "goto child of leaf should error");
        match result {
            Err(DtrError::InvalidState(msg)) => assert!(msg.contains("no child")),
            _ => panic!("expected InvalidState error"),
        }
    }

    #[test]
    fn test_goto_bad_ref() {
        let dir = setup("goto_bad_ref");

        let _input = add_input(&dir, "read_csv('data.csv')", None).expect("input");

        let result = goto(&dir, "nonexistent");
        assert!(result.is_err(), "goto nonexistent ref should error");
        match result {
            Err(DtrError::NodeNotFound(_)) => {}
            _ => panic!("expected NodeNotFound error"),
        }
    }

    #[test]
    fn test_goto_errors_without_cwn() {
        let dir = setup("goto_no_cwn");

        let result = goto(&dir, "..");
        assert!(result.is_err(), "goto with empty CWN should error");
        match result {
            Err(DtrError::NoCurrentNode) => {}
            _ => panic!("expected NoCurrentNode error"),
        }
    }

    // -----------------------------------------------------------------------
    // Add-marker tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_add_marker_to_current_node() {
        let dir = setup("add_marker_current");

        let hash = add_input(&dir, "read_csv('data.csv')", None).expect("add input");
        let result = add_marker(&dir, "mydata").expect("add marker");

        assert_eq!(result, hash, "add_marker should return the node hash");

        let markers = read_markers(&dir).expect("read markers");
        assert_eq!(
            markers.get("mydata"),
            Some(&serde_json::Value::String(hash)),
            "marker should point to the current node"
        );
    }

    #[test]
    fn test_add_marker_overwrites_existing() {
        let dir = setup("add_marker_overwrite");

        let h1 = add_input(&dir, "read_csv('a.csv')", None).expect("first input");
        let _h2 = add_input(&dir, "read_csv('b.csv')", None).expect("second input");

        // CWN is now h2. Add marker pointing to h2.
        add_marker(&dir, "mydata").expect("add marker for h2");

        // Go back to h1 and overwrite the marker
        cwn_write(&dir, &h1).expect("write cwn to h1");
        add_marker(&dir, "mydata").expect("add marker for h1");

        let markers = read_markers(&dir).expect("read markers");
        assert_eq!(
            markers.get("mydata"),
            Some(&serde_json::Value::String(h1)),
            "overwritten marker should point to h1"
        );
    }

    #[test]
    fn test_add_marker_multiple_on_same_node() {
        let dir = setup("add_marker_multiple");

        let hash = add_input(&dir, "read_csv('data.csv')", None).expect("add input");

        add_marker(&dir, "nickname1").expect("first marker");
        add_marker(&dir, "nickname2").expect("second marker");
        add_marker(&dir, "nickname3").expect("third marker");

        let markers = read_markers(&dir).expect("read markers");
        assert_eq!(markers.get("nickname1"), Some(&serde_json::Value::String(hash.clone())));
        assert_eq!(markers.get("nickname2"), Some(&serde_json::Value::String(hash.clone())));
        assert_eq!(markers.get("nickname3"), Some(&serde_json::Value::String(hash.clone())));
    }

    #[test]
    fn test_add_marker_errors_without_cwn() {
        let dir = setup("add_marker_no_cwn");

        let result = add_marker(&dir, "orphan");
        assert!(result.is_err(), "add_marker with empty CWN should error");
        match result {
            Err(DtrError::NoCurrentNode) => {} // expected
            _ => panic!("expected NoCurrentNode error"),
        }
    }

    #[test]
    fn test_add_marker_errors_on_nonexistent_node() {
        let dir = setup("add_marker_bad_hash");

        cwn_write(&dir, "badbadbad").expect("write bad cwn");
        let result = add_marker(&dir, "ghost");
        assert!(result.is_err(), "add_marker with bad CWN should error");
        match result {
            Err(DtrError::NodeNotFound(_)) => {} // expected
            _ => panic!("expected NodeNotFound error"),
        }
    }

    #[test]
    fn test_add_marker_auto_markers_untouched() {
        let dir = setup("add_marker_auto");

        let h1 = add_input(&dir, "read_csv('a.csv')", None).expect("first input");
        let _h2 = add_input(&dir, "read_csv('b.csv')", None).expect("second input");

        // Auto markers exist: input_1, input_2
        // Now add an explicit marker
        add_marker(&dir, "myref").expect("add explicit marker");

        let markers = read_markers(&dir).expect("read markers");
        assert_eq!(markers.get("input_1"), Some(&serde_json::Value::String(h1)));

        let input_2_val = markers.get("input_2").expect("input_2 marker exists");
        assert_eq!(markers.get("myref"), Some(input_2_val));
        assert_eq!(markers.len(), 3, "should have 3 markers total");
    }

    // -----------------------------------------------------------------------
    // Add-lib tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_add_lib_stores_package() {
        let dir = setup("add_lib_basic");

        add_lib(&dir, "dplyr").expect("add dplyr");
        add_lib(&dir, "ggplot2").expect("add ggplot2");

        let packages = packages_read(&dir).expect("read packages");
        assert_eq!(packages, vec!["dplyr", "ggplot2"]);
    }

    #[test]
    fn test_add_lib_deduplicates() {
        let dir = setup("add_lib_dupe");

        add_lib(&dir, "dplyr").expect("first");
        add_lib(&dir, "dplyr").expect("second");

        let packages = packages_read(&dir).expect("read packages");
        assert_eq!(packages, vec!["dplyr"], "duplicates should be ignored");
    }

    #[test]
    fn test_compose_prepends_libraries() {
        let dir = setup("compose_libs");

        add_lib(&dir, "dplyr").expect("add dplyr");
        add_lib(&dir, "ggplot2").expect("add ggplot2");

        add_input(&dir, "read_csv('data.csv')", None).expect("input");

        let result = compose(&dir).expect("compose");
        let expected = "library(dplyr)\nlibrary(ggplot2)\n\nread_csv('data.csv')";
        assert_eq!(result, expected, "should prepend library imports");
    }

    #[test]
    fn test_compose_no_libraries_is_unchanged() {
        let dir = setup("compose_no_libs");

        add_input(&dir, "read_csv('data.csv')", None).expect("input");

        let result = compose(&dir).expect("compose");
        assert_eq!(result, "read_csv('data.csv')");
    }

    // -----------------------------------------------------------------------
    // Read tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_read_returns_code_of_current_node() {
        let dir = setup("read_current");

        let code = "read_csv('data.csv')";
        let hash = add_input(&dir, code, None).expect("add input");

        let result = read_current(&dir).expect("read current");
        assert_eq!(result, code, "should return the R code of current node");
        let _ = hash;
    }

    #[test]
    fn test_read_returns_code_after_chain() {
        let dir = setup("read_chain");

        let _input = add_input(&dir, "read_csv('d.csv')", None).expect("add input");
        let chart_code = "ggplot(aes(x, y)) + geom_point()";
        let _chart = add(&dir, "chart", chart_code, None).expect("add chart");

        let result = read_current(&dir).expect("read current");
        assert_eq!(result, chart_code, "should return chart code");
    }

    #[test]
    fn test_read_errors_without_cwn() {
        let dir = setup("read_no_cwn");

        let result = read_current(&dir);
        assert!(result.is_err(), "read with empty CWN should error");
        match result {
            Err(DtrError::NoCurrentNode) => {}
            _ => panic!("expected NoCurrentNode error"),
        }
    }

    #[test]
    fn test_read_errors_on_nonexistent_node() {
        let dir = setup("read_bad_node");

        cwn_write(&dir, "badbadbad").expect("write bad cwn");
        let result = read_current(&dir);
        assert!(result.is_err(), "read with bad CWN should error");
        match result {
            Err(DtrError::NodeNotFound(_)) => {}
            _ => panic!("expected NodeNotFound error"),
        }
    }

    #[test]
    fn test_read_errors_on_nonexistent_blob() {
        let dir = setup("read_bad_blob");

        // Create a node with a blob hash that doesn't exist in blobs/
        let node = Node {
            node_type: "input".to_string(),
            parents: vec![],
            parent_vars: vec![],
            children: vec![],
            blob: "deadbeef".to_string(),
            cache: None,
        };
        let json = serde_json::to_string(&node).unwrap();
        let node_id = next_node_id(&dir).expect("next id");
        fs::write(dtr_path(&dir).join("nodes").join(&node_id), &json).expect("write node");
        cwn_write(&dir, &node_id).expect("write cwn");

        let result = read_current(&dir);
        assert!(result.is_err(), "read with missing blob should error");
    }

    // -----------------------------------------------------------------------
    // Write tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_write_replaces_code_of_current_node() {
        let dir = setup("write_replace");

        let original = "read_csv('data.csv')";
        let hash = add_input(&dir, original, None).expect("add input");

        let new_code = "read_csv('updated.csv')";
        let new_hash = write_current(&dir, new_code).expect("write current");

        // New code should be readable
        let result = read_current(&dir).expect("read after write");
        assert_eq!(result, new_code, "should return the new code");

        // Node hash should have changed (content changed)
        assert_ne!(new_hash, hash, "write should produce a new node hash");

        // Old blob should still exist (content-addressable, not garbage-collected)
        let old_blob_hash = hash_string(original);
        assert!(dtr_path(&dir).join("blobs").join(&old_blob_hash).exists(), "old blob should persist");

        // New blob should exist
        let new_blob_hash = hash_string(new_code);
        assert!(dtr_path(&dir).join("blobs").join(&new_blob_hash).exists(), "new blob should exist");

        // CWN should point to new node
        let cwn = read_cwn(&dir).expect("read cwn");
        assert_eq!(cwn, new_hash, "CWN should point to new node");
    }

    #[test]
    fn test_write_same_code_is_idempotent() {
        let dir = setup("write_idempotent");

        let code = "read_csv('data.csv')";
        let hash = add_input(&dir, code, None).expect("add input");

        let new_hash = write_current(&dir, code).expect("write same code");
        assert_eq!(new_hash, hash, "writing same code should produce same hash");
    }

    #[test]
    fn test_write_updates_parent_children() {
        let dir = setup("write_parent_child");

        let input_code = "read_csv('data.csv')";
        let input_hash = add_input(&dir, input_code, None).expect("add input");

        let chart_code = "ggplot(aes(x, y)) + geom_point()";
        let chart_hash = add(&dir, "chart", chart_code, None).expect("add chart");

        // Move CWN back to the input node before writing
        cwn_write(&dir, &input_hash).expect("write cwn back to input");

        // Now write new code to the input node
        let new_code = "read_csv('updated.csv')";
        let new_input_hash = write_current(&dir, new_code).expect("write input");

        // CWN is now at new_input_hash. Chart's parent should point to new input hash.
        let chart = read_node(&dir, &chart_hash).expect("read chart");
        assert!(
            chart.parents.contains(&new_input_hash),
            "chart's parent should be updated to new input hash"
        );
        assert!(
            !chart.parents.contains(&input_hash),
            "chart's parent should no longer point to old input hash"
        );

    }

    #[test]
    fn test_write_updates_child_parents() {
        let dir = setup("write_child_parent");

        let input_code = "read_csv('data.csv')";
        let input_hash = add_input(&dir, input_code, None).expect("add input");

        let chart_code = "ggplot(aes(x, y)) + geom_point()";
        let chart_hash = add(&dir, "chart", chart_code, None).expect("add chart");

        // We are at the chart node. Write new code to it.
        let new_code = "ggplot(aes(x, y)) + geom_smooth()";
        let new_chart_hash = write_current(&dir, new_code).expect("write chart");

        // Input's children should now point to the new chart hash
        let input = read_node(&dir, &input_hash).expect("read input");
        assert!(
            input.children.contains(&new_chart_hash),
            "input's children should include new chart hash"
        );
        assert!(
            !input.children.contains(&chart_hash),
            "input's children should not include old chart hash"
        );

        // New chart's parents should point to input
        let new_chart = read_node(&dir, &new_chart_hash).expect("read new chart");
        assert_eq!(new_chart.parents, vec![input_hash], "new chart parent should be input");
    }

    #[test]
    fn test_write_errors_without_cwn() {
        let dir = setup("write_no_cwn");

        let result = write_current(&dir, "read_csv('data.csv')");
        assert!(result.is_err(), "write with empty CWN should error");
        match result {
            Err(DtrError::NoCurrentNode) => {}
            _ => panic!("expected NoCurrentNode error"),
        }
    }

    #[test]
    fn test_write_errors_on_nonexistent_node() {
        let dir = setup("write_bad_node");

        cwn_write(&dir, "badbadbad").expect("write bad cwn");
        let result = write_current(&dir, "read_csv('data.csv')");
        assert!(result.is_err(), "write with bad CWN should error");
        match result {
            Err(DtrError::NodeNotFound(_)) => {}
            _ => panic!("expected NodeNotFound error"),
        }
    }

    #[test]
    fn test_write_clears_own_cache() {
        let dir = setup("write_clear_own");

        let input = add_input(&dir, "read_csv('data.csv')", None).expect("input");

        // Manually set a cache on the input node
        node_cache_set(&dir, &input, "cafecafe").expect("set cache");
        assert!(read_node(&dir, &input).unwrap().cache.is_some(), "should have cache");

        // Write new code — should clear the cache
        write_current(&dir, "read_csv('other.csv')").expect("write");

        let new_hash = read_cwn(&dir).unwrap();
        let node = read_node(&dir, &new_hash).unwrap();
        assert!(node.cache.is_none(), "cache should be cleared after write");
    }

    #[test]
    fn test_write_clears_descendant_caches() {
        let dir = setup("write_clear_descendants");

        let input = add_input(&dir, "read_csv('data.csv')", None).expect("input");
        let process = add(&dir, "process", "filter(x > 0)", None).expect("process");
        let chart = add(&dir, "chart", "ggplot()", None).expect("chart");

        // Cache the process and chart nodes
        node_cache_set(&dir, &process, "abc123").expect("cache process");
        node_cache_set(&dir, &chart, "def456").expect("cache chart");

        // Go back to input and write new code
        cwn_write(&dir, &input).expect("goto input");
        write_current(&dir, "read_csv('updated.csv')").expect("write input");

        // Process and chart should have their caches cleared
        let process_node = read_node(&dir, &process).unwrap();
        assert!(process_node.cache.is_none(), "process cache should be cleared");

        let chart_node = read_node(&dir, &chart).unwrap();
        assert!(chart_node.cache.is_none(), "chart cache should be cleared");
    }

    #[test]
    fn test_write_clears_full_tree() {
        let dir = setup("write_clear_tree");

        // Build: input → process → chart, and input → process → model (branch)
        let input = add_input(&dir, "read_csv('data.csv')", None).expect("input");
        let process = add(&dir, "process", "filter(x > 0)", None).expect("process");
        let chart = add(&dir, "chart", "ggplot()", None).expect("chart");

        cwn_write(&dir, &process).expect("goto process");
        let model = add(&dir, "model", "glm(y ~ x)", None).expect("model");

        // Cache the process and both children
        node_cache_set(&dir, &process, "aaa").expect("cache process");
        node_cache_set(&dir, &chart, "bbb").expect("cache chart");
        node_cache_set(&dir, &model, "ccc").expect("cache model");

        // Edit the input node
        cwn_write(&dir, &input).expect("goto input");
        write_current(&dir, "read_csv('updated.csv')").expect("write input");

        // All caches downstream should be cleared
        assert!(read_node(&dir, &process).unwrap().cache.is_none());
        assert!(read_node(&dir, &chart).unwrap().cache.is_none());
        assert!(read_node(&dir, &model).unwrap().cache.is_none());
    }

    // -----------------------------------------------------------------------
    // Cache tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_cache_runs_r_and_stores_rds() {
        let dir = setup("cache_rds");

        let hash = add_input(&dir, "head(mtcars, 2)", None).expect("add input");

        let cache_hash = cache(&dir).expect("cache");

        // Node's cache field should be set
        let node = read_node(&dir, &hash).expect("read node");
        assert_eq!(node.cache, Some(cache_hash.clone()), "node cache field should be set");

        // Cache file should exist (RDS binary, content-addressed)
        let cache_path = dtr_path(&dir).join("cache").join(&cache_hash);
        assert!(cache_path.exists(), "cache file should exist");
        let rds_bytes = fs::read(&cache_path).expect("read rds");
        assert!(!rds_bytes.is_empty(), "RDS should not be empty");
        assert_eq!(cache_hash, hash_bytes(&rds_bytes), "cache hash should match RDS content");
    }

    #[test]
    fn test_cache_overwrite_updates_node() {
        let dir = setup("cache_overwrite");

        let _old_hash = add_input(&dir, "head(mtcars, 2)", None).expect("add input");
        let h1 = cache(&dir).expect("cache v1");

        // Write new code (invalidates old cache) and cache again
        write_current(&dir, "head(mtcars, 5)").expect("write");
        let new_hash = read_cwn(&dir).unwrap();
        let h2 = cache(&dir).expect("cache v2");

        assert_ne!(h1, h2, "different outputs should have different hashes");
        assert_eq!(read_node(&dir, &new_hash).unwrap().cache, Some(h2.clone()));

        // Both cache files should exist (immutable, content-addressed)
        assert!(dtr_path(&dir).join("cache").join(&h1).exists());
        assert!(dtr_path(&dir).join("cache").join(&h2).exists());
    }

    #[test]
    fn test_cache_errors_without_cwn() {
        let dir = setup("cache_no_cwn");

        let result = cache(&dir);
        assert!(result.is_err(), "cache with empty CWN should error");
        match result {
            Err(DtrError::NoCurrentNode) => {}
            _ => panic!("expected NoCurrentNode error"),
        }
    }

    #[test]
    fn test_cache_errors_on_nonexistent_node() {
        let dir = setup("cache_bad_node");

        cwn_write(&dir, "deadbeef").expect("write bad cwn");
        let result = cache(&dir);
        assert!(result.is_err(), "cache with bad CWN should error");
        match result {
            Err(DtrError::NodeNotFound(_)) => {}
            _ => panic!("expected NodeNotFound error"),
        }
    }

    // -----------------------------------------------------------------------
    // Preview tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_preview_errors_without_cache() {
        let dir = setup("preview_no_cache");

        // Create a node with no cache
        let hash = add_input(&dir, "head(mtcars, 2)", None).expect("add input");

        let result = preview(&dir, Some(&hash));
        assert!(result.is_err(), "preview without cache should error");
        match result {
            Err(DtrError::InvalidState(msg)) => {
                assert!(msg.contains("no cached output"), "should mention cache");
            }
            _ => panic!("expected InvalidState error"),
        }
    }

    #[test]
    fn test_preview_errors_without_cwn() {
        let dir = setup("preview_no_cwn");

        let result = preview(&dir, None);
        assert!(result.is_err(), "preview with empty CWN should error");
        match result {
            Err(DtrError::NoCurrentNode) => {}
            _ => panic!("expected NoCurrentNode error"),
        }
    }

    #[test]
    fn test_preview_errors_on_nonexistent_target() {
        let dir = setup("preview_bad_target");

        let result = preview(&dir, Some("nonexistent"));
        assert!(result.is_err(), "preview with bad target should error");
        match result {
            Err(DtrError::NodeNotFound(_)) => {}
            _ => panic!("expected NodeNotFound error"),
        }
    }

    #[test]
    fn test_preview_by_marker_finds_node() {
        let dir = setup("preview_by_marker");

        let hash = add_input(&dir, "head(mtcars, 2)", Some("mydata")).expect("add input");

        // Set a fake cache
        fs::create_dir_all(dtr_path(&dir).join("cache")).expect("mkdir");
        fs::write(dtr_path(&dir).join("cache").join("abc"), "fake rds").expect("write");
        node_cache_set(&dir, &hash, "abc").expect("set cache");

        // Preview by marker name — should resolve and try to run R (will fail
        // since RDS isn't real, but the resolution and cache check pass)
        let result = preview(&dir, Some("mydata"));
        // This will likely fail at R execution since the RDS is fake,
        // but it should NOT fail with NodeNotFound or NoCurrentNode
        assert!(
            !matches!(&result, Err(DtrError::NodeNotFound(_))),
            "marker should resolve"
        );
        assert!(
            !matches!(&result, Err(DtrError::NoCurrentNode)),
            "should not need CWN"
        );
    }

    #[test]
    fn test_build_preview_r_script_text_object() {
        let libs = "library(dplyr)\n\n";
        let cache_path = std::path::Path::new("/tmp/cache_abc123");
        let png_path = std::path::Path::new("/tmp/preview.png");
        let script = build_preview_r_script(libs, cache_path, png_path);

        assert!(script.contains("library(dplyr)"), "should include library imports");
        assert!(script.contains("readRDS('/tmp/cache_abc123')"), "should read RDS");
        assert!(script.contains("inherits(obj, 'ggplot')"), "should check for ggplot");
        assert!(script.contains("inherits(obj, 'data.frame')"), "should check for data.frame");
        assert!(script.contains("inherits(obj, 'lm')"), "should check for lm");
        assert!(script.contains("inherits(obj, 'glm')"), "should check for glm");
        assert!(script.contains("__DTR_PNG__"), "should emit PNG marker");
    }

    #[test]
    fn test_build_preview_r_script_includes_ggsave() {
        let libs = "";
        let cache_path = std::path::Path::new("/tmp/cache_xxx");
        let png_path = std::path::Path::new("/tmp/out.png");
        let script = build_preview_r_script(libs, cache_path, png_path);

        assert!(script.contains("ggsave('/tmp/out.png'"), "should save to png path");
        assert!(script.contains("print(head(obj, 20))"), "should preview data frames");
        assert!(script.contains("print(summary(obj))"), "should summarize models");
    }

    // -----------------------------------------------------------------------
    // Run tests (script generation, no R required)
    // -----------------------------------------------------------------------

    #[test]
    fn test_compose_node_cached_without_cache() {
        let dir = setup("run_no_cache");

        let _input = add_input(&dir, "read_csv('data.csv')", None).expect("input");
        let _chart = add(&dir, "chart", "ggplot()", None).expect("chart");

        // No cache set — should compose full chain
        let cwn = read_cwn(&dir).unwrap();
        let script = compose_node_cached(&dir, &cwn).expect("compose cached");
        assert_eq!(
            script,
            "read_csv('data.csv') |>\n  ggplot()",
            "without cache should compose full chain"
        );
    }

    #[test]
    fn test_compose_node_cached_stops_at_cached_ancestor() {
        let dir = setup("run_with_cache");

        let input = add_input(&dir, "read_csv('data.csv')", None).expect("input");

        // Manually cache the input node
        node_cache_set(&dir, &input, "cafecafe").expect("set cache");

        // Create a fake cache file
        fs::create_dir_all(dtr_path(&dir).join("cache")).expect("mkdir cache");
        fs::write(dtr_path(&dir).join("cache").join("cafecafe"), "fake rds").expect("write cache");

        let _chart = add(&dir, "chart", "ggplot()", None).expect("chart");

        // Should use readRDS instead of read_csv
        let cwn = read_cwn(&dir).unwrap();
        let script = compose_node_cached(&dir, &cwn).expect("compose cached");

        let expected = format!(
            "readRDS('{}') |>\n  ggplot()",
            dtr_path(&dir).join("cache").join("cafecafe").display()
        );
        assert_eq!(script, expected);
    }

    #[test]
    fn test_compose_node_cached_ignores_missing_cache_file() {
        let dir = setup("run_bad_cache");

        let input = add_input(&dir, "read_csv('data.csv')", None).expect("input");

        // Set cache hash but don't create the file
        node_cache_set(&dir, &input, "deadbeef").expect("set cache");

        let _chart = add(&dir, "chart", "ggplot()", None).expect("chart");

        let cwn = read_cwn(&dir).unwrap();
        let script = compose_node_cached(&dir, &cwn).expect("compose cached");
        // Should fall through to full composition since cache file is missing
        assert_eq!(
            script,
            "read_csv('data.csv') |>\n  ggplot()",
            "missing cache file should fall through"
        );
    }

    #[test]
    fn test_compose_node_cached_mid_chain() {
        let dir = setup("run_mid_chain");

        let _input = add_input(&dir, "read_csv('data.csv')", None).expect("input");
        let proc_hash = add(&dir, "process", "filter(x > 0)", None).expect("process");
        let _chart = add(&dir, "chart", "ggplot()", None).expect("chart");

        // Cache the process node (mid-chain)
        fs::create_dir_all(dtr_path(&dir).join("cache")).expect("mkdir");
        fs::write(dtr_path(&dir).join("cache").join("abc123"), "rds").expect("write");
        node_cache_set(&dir, &proc_hash, "abc123").expect("set cache");

        let cwn = read_cwn(&dir).unwrap();
        let script = compose_node_cached(&dir, &cwn).expect("compose cached");

        // Should compose: readRDS(cache/abc123) |> ggplot() — skipping input and process
        let expected = format!(
            "readRDS('{}') |>\n  ggplot()",
            dtr_path(&dir).join("cache").join("abc123").display()
        );
        assert_eq!(script, expected);
    }

    #[test]
    fn test_wrap_r_script_builds_correctly() {
        let composed = "read_csv('data.csv') |>\n  filter(x > 0)";
        let rds_path = std::path::Path::new("/tmp/test.rds");
        let script = wrap_r_script("", composed, rds_path);

        // Chain is wrapped in braces so multi-statement merge output works
        assert!(script.starts_with("result <- {"), "should assign to result via braces");
        assert!(script.contains("read_csv"), "should contain composed code");
        assert!(script.contains("print(result)"), "should print result");
        assert!(
            script.contains("saveRDS(result, '/tmp/test.rds')"),
            "should save RDS: got\n{script}"
        );
    }

    #[test]
    fn test_wrap_r_script_with_libraries() {
        let libs = "library(dplyr)\nlibrary(ggplot2)\n\n";
        let composed = "read_csv('data.csv') |>\n  ggplot(aes(x, y)) + geom_point()";
        let rds_path = std::path::Path::new("/tmp/test.rds");
        let script = wrap_r_script(libs, composed, rds_path);

        // Library calls must come BEFORE result <-
        let result_pos = script.find("result <-").unwrap();
        let lib_pos = script.find("library(dplyr)").unwrap();
        assert!(lib_pos < result_pos, "library() must precede result <-");

        // Chain is wrapped in braces: result <- {\n...\n}
        assert!(
            script.contains("result <- {"),
            "result should use brace block: got\n{script}"
        );
        assert!(script.contains("print(result)"), "should print result");
        assert!(
            script.contains("saveRDS(result, '/tmp/test.rds')"),
            "should save RDS"
        );
    }

    #[test]
    fn test_wrap_r_script_with_merge_multi_statement() {
        let libs = "library(dplyr)\n\n";
        // Simulate what compose_node emits for a merge node:
        // side-parent assignment + main pipe chain
        let composed = concat!(
            "b <- read_csv('b.csv')\n",
            "\n",
            "read_csv('a.csv') |>\n",
            "  left_join(b, by = 'id') |>\n",
            "  mutate(z = x + y)"
        );
        let rds_path = std::path::Path::new("/tmp/test.rds");
        let script = wrap_r_script(libs, composed, rds_path);

        // The entire composed chain must be inside braces
        assert!(script.contains("result <- {"), "should open brace block");
        assert!(script.contains("\n}\nprint(result)"), "should close brace before print");

        // Library before result
        let result_pos = script.find("result <-").unwrap();
        let lib_pos = script.find("library(dplyr)").unwrap();
        assert!(lib_pos < result_pos, "library() must precede result <-");

        // Must contain both the assignment and the pipe chain
        assert!(script.contains("b <- read_csv"), "should contain side assignment");
        assert!(script.contains("left_join(b, by = 'id')"), "should contain merge");
        assert!(script.contains("mutate(z = x + y)"), "should contain downstream transform");
    }

    #[test]
    fn test_run_errors_without_cwn() {
        let dir = setup("run_no_cwn");

        let result = run(&dir, false);
        assert!(result.is_err(), "run with empty CWN should error");
        match result {
            Err(DtrError::NoCurrentNode) => {}
            _ => panic!("expected NoCurrentNode error"),
        }
    }

    #[test]
    fn test_run_errors_on_nonexistent_node() {
        let dir = setup("run_bad_node");

        cwn_write(&dir, "deadbeef").expect("write bad cwn");
        let result = run(&dir, false);
        assert!(result.is_err(), "run with bad CWN should error");
        match result {
            Err(DtrError::NodeNotFound(_)) => {}
            _ => panic!("expected NodeNotFound error"),
        }
    }

    // Helper: manually set a node's cache field
    fn node_cache_set(dir: &Path, hash: &str, cache_hash: &str) -> Result<(), DtrError> {
        let mut node = node_read(dir, hash)?;
        node.cache = Some(cache_hash.to_string());
        fs::write(
            dtr_path(dir).join("nodes").join(hash),
            serde_json::to_string_pretty(&node)?,
        )?;
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Delete tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_delete_removes_node_and_moves_to_parent() {
        let dir = setup("delete_to_parent");

        let input = add_input(&dir, "read_csv('data.csv')", None).expect("add input");
        let chart = add(&dir, "chart", "ggplot(aes(x, y)) + geom_point()", None).expect("add chart");

        // CWN is chart. Delete it.
        delete_current(&dir, false).expect("delete chart");

        // Chart node should no longer exist
        let chart_result = read_node(&dir, &chart);
        assert!(chart_result.is_err(), "chart node should be deleted");

        // CWN should be back at input
        let cwn = read_cwn(&dir).expect("read cwn");
        assert_eq!(cwn, input, "CWN should move to parent after delete");

        // Input's children should no longer include chart
        let input_node = read_node(&dir, &input).expect("read input");
        assert!(!input_node.children.contains(&chart), "input should no longer list chart as child");
    }

    #[test]
    fn test_delete_root_node_clears_cwn() {
        let dir = setup("delete_root");

        let hash = add_input(&dir, "read_csv('data.csv')", None).expect("add input");

        delete_current(&dir, false).expect("delete root");

        // Node should be gone
        let node_result = read_node(&dir, &hash);
        assert!(node_result.is_err(), "root node should be deleted");

        // CWN should be empty
        let cwn = read_cwn(&dir);
        assert!(cwn.is_err(), "CWN should be empty after deleting root");
        match cwn {
            Err(DtrError::NoCurrentNode) => {}
            _ => panic!("expected NoCurrentNode"),
        }
    }

    #[test]
    fn test_delete_errors_with_children() {
        let dir = setup("delete_with_children");

        let _input = add_input(&dir, "read_csv('data.csv')", None).expect("add input");
        let _chart = add(&dir, "chart", "ggplot(aes(x, y)) + geom_point()", None).expect("add chart");

        // Go back to input (which now has a child)
        let _ = read_cwn(&dir); // CWN is at chart
        // We need to move CWN to input first
        // Actually input_hash is lost... let's get it from chart's parents
        let cwn = read_cwn(&dir).expect("cwn at chart");
        let chart_node = read_node(&dir, &cwn).expect("read chart");
        let input_hash = chart_node.parents[0].clone();
        cwn_write(&dir, &input_hash).expect("move to input");

        // Now try to delete input (which has a child)
        let result = delete_current(&dir, false);
        assert!(result.is_err(), "delete with children should error");
        match result {
            Err(DtrError::InvalidState(msg)) => {
                assert!(msg.contains("children"), "error should mention children");
            }
            _ => panic!("expected InvalidState error"),
        }

        // Input node should still exist
        let input_node = read_node(&dir, &input_hash).expect("input should still exist");
        assert!(!input_node.children.is_empty(), "input should still have children");
    }

    #[test]
    fn test_delete_recursive_removes_chain() {
        let dir = setup("delete_recursive_chain");

        let input = add_input(&dir, "read_csv('data.csv')", None).expect("add input");
        let chart = add(&dir, "chart", "ggplot(aes(x, y)) + geom_point()", None).expect("add chart");
        let model = add(&dir, "model", "glm(y ~ x)", None).expect("add model");

        // Now at model. Go back to input and delete recursively.
        cwn_write(&dir, &input).expect("move to input");
        delete_current(&dir, true).expect("recursive delete");

        // All nodes should be gone
        assert!(read_node(&dir, &input).is_err(), "input should be deleted");
        assert!(read_node(&dir, &chart).is_err(), "chart should be deleted");
        assert!(read_node(&dir, &model).is_err(), "model should be deleted");

        // CWN should be empty
        let cwn = read_cwn(&dir);
        assert!(cwn.is_err(), "CWN should be empty");
    }

    #[test]
    fn test_delete_recursive_with_branch() {
        let dir = setup("delete_recursive_branch");

        // Create: input -> chart, input -> model (branching)
        let input = add_input(&dir, "read_csv('data.csv')", None).expect("add input");
        let chart = add(&dir, "chart", "ggplot(aes(x, y)) + geom_point()", None).expect("add chart");
        cwn_write(&dir, &input).expect("back to input");
        let model = add(&dir, "model", "glm(y ~ x)", None).expect("add model");

        // Delete input recursively
        cwn_write(&dir, &input).expect("move to input");
        delete_current(&dir, true).expect("recursive delete");

        // Everything should be gone
        assert!(read_node(&dir, &input).is_err(), "input deleted");
        assert!(read_node(&dir, &chart).is_err(), "chart deleted");
        assert!(read_node(&dir, &model).is_err(), "model deleted");

        // CWN should be empty
        assert!(read_cwn(&dir).is_err(), "CWN empty");
    }

    #[test]
    fn test_delete_errors_without_cwn() {
        let dir = setup("delete_no_cwn");

        let result = delete_current(&dir, false);
        assert!(result.is_err(), "delete with empty CWN should error");
        match result {
            Err(DtrError::NoCurrentNode) => {}
            _ => panic!("expected NoCurrentNode error"),
        }
    }

    #[test]
    fn test_delete_errors_on_nonexistent_node() {
        let dir = setup("delete_bad_node");

        cwn_write(&dir, "badbadbad").expect("write bad cwn");
        let result = delete_current(&dir, false);
        assert!(result.is_err(), "delete with bad CWN should error");
        match result {
            Err(DtrError::NodeNotFound(_)) => {}
            _ => panic!("expected NodeNotFound error"),
        }
    }

    #[test]
    fn test_delete_parent_no_longer_lists_child() {
        let dir = setup("delete_parent_refs");

        let input = add_input(&dir, "read_csv('data.csv')", None).expect("add input");
        let chart = add(&dir, "chart", "ggplot(aes(x, y)) + geom_point()", None).expect("add chart");

        // Go back to input and delete chart
        cwn_write(&dir, &input).expect("move to input");
        // Actually we want to delete chart, not input. Let's set CWN to chart.
        cwn_write(&dir, &chart).expect("move to chart");
        delete_current(&dir, false).expect("delete chart");

        // Input's children should no longer include chart
        let input_node = read_node(&dir, &input).expect("read input");
        assert!(
            !input_node.children.contains(&chart),
            "input should no longer list chart as child"
        );

        // CWN should be back at input
        let cwn = read_cwn(&dir).expect("read cwn");
        assert_eq!(cwn, input, "CWN should move to parent");
    }

    #[test]
    fn test_delete_cleans_up_cache() {
        let dir = setup("delete_cache");

        let hash = add_input(&dir, "read_csv('data.csv')", None).expect("input");

        // Set a fake cache on the node
        let cache_hash = "cafecafe";
        fs::create_dir_all(dtr_path(&dir).join("cache")).expect("mkdir");
        fs::write(dtr_path(&dir).join("cache").join(cache_hash), "fake rds").expect("write");
        node_cache_set(&dir, &hash, cache_hash).expect("set cache");

        // Delete the node
        delete_current(&dir, false).expect("delete");

        // Cache file should be gone
        assert!(
            !dtr_path(&dir).join("cache").join(cache_hash).exists(),
            "cache file should be deleted"
        );
    }

    #[test]
    fn test_delete_recursive_cleans_up_all_caches() {
        let dir = setup("delete_recursive_cache");

        let input = add_input(&dir, "read_csv('data.csv')", None).expect("input");
        let process = add(&dir, "process", "filter(x > 0)", None).expect("process");
        let chart = add(&dir, "chart", "ggplot()", None).expect("chart");

        // Cache all three nodes
        fs::create_dir_all(dtr_path(&dir).join("cache")).expect("mkdir");
        fs::write(dtr_path(&dir).join("cache").join("aaa"), "rds").expect("write");
        fs::write(dtr_path(&dir).join("cache").join("bbb"), "rds").expect("write");
        fs::write(dtr_path(&dir).join("cache").join("ccc"), "rds").expect("write");
        node_cache_set(&dir, &input, "aaa").expect("cache input");
        node_cache_set(&dir, &process, "bbb").expect("cache process");
        node_cache_set(&dir, &chart, "ccc").expect("cache chart");

        // Delete input recursively
        cwn_write(&dir, &input).expect("goto input");
        delete_current(&dir, true).expect("recursive delete");

        // All cache files should be gone
        assert!(!dtr_path(&dir).join("cache").join("aaa").exists(), "aaa should be deleted");
        assert!(!dtr_path(&dir).join("cache").join("bbb").exists(), "bbb should be deleted");
        assert!(!dtr_path(&dir).join("cache").join("ccc").exists(), "ccc should be deleted");
    }

    // -----------------------------------------------------------------------
    // Compose tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_compose_single_input_node() {
        let dir = setup("compose_single_input");

        let code = "read_csv('data.csv')";
        add_input(&dir, code, None).expect("add input");

        let result = compose(&dir).expect("compose");
        assert_eq!(result, code, "single input node should return just its code");
    }

    #[test]
    fn test_compose_linear_chain() {
        let dir = setup("compose_linear");

        let _input = add_input(&dir, "read_csv('data.csv')", None).expect("add input");
        let _filter = add(&dir, "process", "filter(x > 0)", None).expect("add filter");
        let _chart = add(&dir, "chart", "ggplot(aes(x, y)) + geom_point()", None).expect("add chart");

        let result = compose(&dir).expect("compose");
        let expected = "read_csv('data.csv') |>\n  filter(x > 0) |>\n  ggplot(aes(x, y)) + geom_point()";
        assert_eq!(result, expected, "linear chain should produce chained pipe script");
    }

    #[test]
    fn test_compose_input_to_process() {
        let dir = setup("compose_input_process");

        let _input = add_input(&dir, "read_csv('data.csv')", None).expect("add input");
        let _mut = add(&dir, "process", "mutate(z = x + y)", None).expect("add mutate");

        let result = compose(&dir).expect("compose");
        assert_eq!(
            result,
            "read_csv('data.csv') |>\n  mutate(z = x + y)",
            "input -> process should chain correctly"
        );
    }

    #[test]
    fn test_compose_input_to_model() {
        let dir = setup("compose_input_model");

        let _input = add_input(&dir, "read_csv('data.csv')", None).expect("add input");
        let _model = add(&dir, "model", "glm(y ~ x, family = binomial)", None).expect("add model");

        let result = compose(&dir).expect("compose");
        assert_eq!(
            result,
            "read_csv('data.csv') |>\n  glm(y ~ x, family = binomial)",
            "input -> model should chain correctly"
        );
    }

    #[test]
    fn test_compose_merge_with_markers() {
        let dir = setup("compose_merge_markers");

        let left_hash = add_input(&dir, "read_csv('left.csv')", Some("left")).expect("left input");
        let right_hash = add_input(&dir, "read_csv('right.csv')", Some("right")).expect("right input");

        let merge_code = "inner_join(right, by = 'id')";
        add_merge(&dir, merge_code, &[&left_hash, &right_hash], None).expect("add merge");

        let result = compose(&dir).expect("compose");
        let expected = "right <- read_csv('right.csv')\n\nread_csv('left.csv') |>\n  inner_join(right, by = 'id')";
        assert_eq!(result, expected, "merge should pre-assign right branch by marker name");
    }

    #[test]
    fn test_compose_merge_without_markers() {
        let dir = setup("compose_merge_no_markers");

        let left = add_input(&dir, "read_csv('left.csv')", None).expect("left input");
        let right = add_input(&dir, "read_csv('right.csv')", None).expect("right input");

        // Clear markers to force hash-based fallback naming
        fs::write(dtr_path(&dir).join("markers"), "{}\n").expect("clear markers");

        // The variable name is derived from the right input's hash at merge creation
        let right_var = format!("p_{}", &right[..8.min(right.len())]);
        let merge_code = format!("inner_join({right_var}, by = 'id')");
        add_merge(&dir, &merge_code, &[&left, &right], None).expect("add merge");

        let result = compose(&dir).expect("compose");
        let expected = format!(
            "{right_var} <- read_csv('right.csv')\n\nread_csv('left.csv') |>\n  inner_join({right_var}, by = 'id')"
        );
        assert_eq!(result, expected, "side branch without marker should get hash-based var name");
    }

    #[test]
    fn test_compose_merge_with_processing_on_both_sides() {
        let dir = setup("compose_merge_processed");

        let left = add_input(&dir, "read_csv('left.csv')", Some("left")).expect("left input");
        let right = add_input(&dir, "read_csv('right.csv')", Some("right")).expect("right input");
        let _ = right;

        // Process the right branch, then give the processed node a marker
        let _right_proc = add(&dir, "process", "filter(x > 0)", None).expect("right filter");
        let right_proc_hash = read_cwn(&dir).expect("read cwn");
        add_marker(&dir, "right_filtered").expect("mark processed right");

        // Merge: left as primary, processed-right as side
        let merge_code = "inner_join(right_filtered, by = 'id')";
        add_merge(&dir, merge_code, &[&left, &right_proc_hash], None).expect("add merge");

        let result = compose(&dir).expect("compose");
        let expected = "right_filtered <- read_csv('right.csv') |>\n  filter(x > 0)\n\nread_csv('left.csv') |>\n  inner_join(right_filtered, by = 'id')";
        assert_eq!(result, expected, "side branch with processing should chain into its marker-named variable");
    }

    #[test]
    fn test_compose_merge_then_chart() {
        let dir = setup("compose_merge_chart");

        let left = add_input(&dir, "read_csv('left.csv')", Some("left")).expect("left");
        let right = add_input(&dir, "read_csv('right.csv')", Some("right")).expect("right");
        add_merge(&dir, "inner_join(right, by = 'id')", &[&left, &right], None).expect("merge");
        add(&dir, "chart", "ggplot(aes(x, y)) + geom_point()", None).expect("chart");

        let result = compose(&dir).expect("compose");
        let expected = concat!(
            "right <- read_csv('right.csv')\n",
            "\n",
            "read_csv('left.csv') |>\n",
            "  inner_join(right, by = 'id') |>\n",
            "  ggplot(aes(x, y)) + geom_point()",
        );
        assert_eq!(result, expected, "merge -> chart should chain the merge result");
    }

    #[test]
    fn test_compose_deep_chain_with_process() {
        let dir = setup("compose_deep");

        let _input = add_input(&dir, "read_csv('data.csv')", None).expect("input");
        let _f1 = add(&dir, "process", "filter(x > 0)", None).expect("filter");
        let _mut = add(&dir, "process", "mutate(z = x + y)", None).expect("mutate");
        let _sel = add(&dir, "process", "select(x, y, z)", None).expect("select");
        let _chart = add(&dir, "chart", "ggplot(aes(x, z)) + geom_point()", None).expect("chart");

        let result = compose(&dir).expect("compose");
        let expected = concat!(
            "read_csv('data.csv') |>\n",
            "  filter(x > 0) |>\n",
            "  mutate(z = x + y) |>\n",
            "  select(x, y, z) |>\n",
            "  ggplot(aes(x, z)) + geom_point()",
        );
        assert_eq!(result, expected, "deep chain should produce correct pipe nesting");
    }

    #[test]
    fn test_compose_errors_without_cwn() {
        let dir = setup("compose_no_cwn");

        let result = compose(&dir);
        assert!(result.is_err(), "compose with empty CWN should error");
        match result {
            Err(DtrError::NoCurrentNode) => {}
            _ => panic!("expected NoCurrentNode error"),
        }
    }

    #[test]
    fn test_compose_errors_on_nonexistent_node() {
        let dir = setup("compose_bad_node");

        cwn_write(&dir, "badbadbad").expect("write bad cwn");
        let result = compose(&dir);
        assert!(result.is_err(), "compose with bad CWN should error");
        match result {
            Err(DtrError::NodeNotFound(_)) => {}
            _ => panic!("expected NodeNotFound error"),
        }
    }

    #[test]
    fn test_compose_preserves_parent_order_in_merge() {
        let dir = setup("compose_merge_order");

        let a = add_input(&dir, "read_csv('a.csv')", Some("first")).expect("a");
        let b = add_input(&dir, "read_csv('b.csv')", Some("second")).expect("b");
        let c = add_input(&dir, "read_csv('c.csv')", Some("third")).expect("c");

        // Merge with 3 parents: a is primary, b and c are side
        add_merge(&dir, "inner_join(second, by='id', None) |> inner_join(third, by='x')", &[&a, &b, &c], None)
            .expect("merge");

        let result = compose(&dir).expect("compose");
        // Side branches should be emitted in order (b then c), both before main chain
        assert!(
            result.contains("second <- read_csv('b.csv')"),
            "should pre-compute second branch: got\n{result}"
        );
        assert!(
            result.contains("third <- read_csv('c.csv')"),
            "should pre-compute third branch: got\n{result}"
        );
        // second should appear before third in the output
        let second_pos = result.find("second <-").unwrap();
        let third_pos = result.find("third <-").unwrap();
        assert!(second_pos < third_pos, "second branch should come before third");
        // Main chain (first) should appear last
        let first_pos = result.find("read_csv('a.csv')").unwrap();
        assert!(first_pos > third_pos, "primary parent should come after side branches");
    }

    #[test]
    fn test_compose_nested_merges_no_collision() {
        let dir = setup("compose_nested_merge");

        // Build a DAG with nested merges:
        //   a ──→ merge1 ──→ merge3
        //   b ──↗          ↗
        //   c ────────────↗
        // merge1: inner_join(b, by='id')
        // merge3: inner_join(c, by='x')
        // Both b and c have markers, so variable names are distinct

        let a = add_input(&dir, "read_csv('a.csv')", Some("a")).expect("a");
        let b = add_input(&dir, "read_csv('b.csv')", Some("b")).expect("b");
        let c = add_input(&dir, "read_csv('c.csv')", Some("c")).expect("c");

        // merge1 uses b as side parent
        add_merge(&dir, "inner_join(b, by='id')", &[&a, &b], None).expect("merge1");
        let merge1_hash = read_cwn(&dir).expect("read cwn");

        // merge3 uses c as side parent, merge1 as primary
        add_merge(&dir, "inner_join(c, by='x')", &[&merge1_hash, &c], None).expect("merge3");

        let result = compose(&dir).expect("compose");

        // Both b and c should be pre-computed with their marker names
        assert!(
            result.contains("b <- read_csv('b.csv')"),
            "b should be pre-computed: got\n{result}"
        );
        assert!(
            result.contains("c <- read_csv('c.csv')"),
            "c should be pre-computed: got\n{result}"
        );

        // Both merges should use the correct variable names
        assert!(result.contains("inner_join(b,"), "merge1 should reference b");
        assert!(result.contains("inner_join(c,"), "merge3 should reference c");

        // The main chain should flow: a -> merge1 -> merge3
        assert!(
            result.contains("read_csv('a.csv') |>"),
            "a should pipe into merge1"
        );
        assert!(
            result.contains("inner_join(b, by='id') |>"),
            "merge1 should pipe into merge3"
        );
    }

    #[test]
    fn test_compose_nested_merges_unmarked_no_collision() {
        let dir = setup("compose_nested_unmarked");

        // Same DAG as above, but with no markers — forces hash-based naming.
        // This is the case that would have silently produced wrong output before.
        let a = add_input(&dir, "read_csv('a.csv')", None).expect("a");
        let b = add_input(&dir, "read_csv('b.csv')", None).expect("b");
        let c = add_input(&dir, "read_csv('c.csv')", None).expect("c");

        // Clear markers to force hash-based fallback
        fs::write(dtr_path(&dir).join("markers"), "{}\n").expect("clear markers");

        // Variable names derive from hashes at creation time
        let b_var = format!("p_{}", &b[..8.min(b.len())]);
        let c_var = format!("p_{}", &c[..8.min(c.len())]);

        // merge1: b is side parent
        let merge1_code = format!("inner_join({b_var}, by='id')");
        add_merge(&dir, &merge1_code, &[&a, &b], None).expect("merge1");
        let merge1_hash = read_cwn(&dir).expect("read cwn");

        // merge3: c is side parent, merge1 is primary
        let merge3_code = format!("inner_join({c_var}, by='x')");
        add_merge(&dir, &merge3_code, &[&merge1_hash, &c], None).expect("merge3");

        let result = compose(&dir).expect("compose");

        // Both hash-based variables must appear with DIFFERENT names
        assert!(
            result.contains(&format!("{b_var} <- read_csv('b.csv')")),
            "b should use {b_var}: got\n{result}"
        );
        assert!(
            result.contains(&format!("{c_var} <- read_csv('c.csv')")),
            "c should use {c_var}: got\n{result}"
        );

        // Verify the names are different (the whole point of the fix)
        assert_ne!(b_var, c_var, "nested merge variables must be distinct");

        // Each merge references the correct variable
        assert!(
            result.contains(&format!("inner_join({b_var},")),
            "merge1 should reference {b_var}"
        );
        assert!(
            result.contains(&format!("inner_join({c_var},")),
            "merge3 should reference {c_var}"
        );
    }

    // -----------------------------------------------------------------------
    // Hash consistency
    // -----------------------------------------------------------------------

    #[test]
    fn test_same_code_same_blob_hash() {
        let dir = setup("same_blob_hash");

        let h1 = add_input(&dir, "read_csv('data.csv')", Some("a")).expect("first");
        let h2 = add_input(&dir, "read_csv('data.csv')", Some("b")).expect("second");

        // Same code should produce same blob hash
        let n1 = read_node(&dir, &h1).expect("node a");
        let n2 = read_node(&dir, &h2).expect("node b");
        assert_eq!(n1.blob, n2.blob, "same code should produce same blob hash");
    }

    #[test]
    fn test_different_code_different_blob_hash() {
        let dir = setup("diff_blob_hash");

        let h1 = add_input(&dir, "read_csv('a.csv')", Some("a")).expect("first");
        let h2 = add_input(&dir, "read_csv('b.csv')", Some("b")).expect("second");

        let n1 = read_node(&dir, &h1).expect("node a");
        let n2 = read_node(&dir, &h2).expect("node b");
        assert_ne!(n1.blob, n2.blob, "different code should produce different blob hashes");
    }

    // -----------------------------------------------------------------------
    // Node structure invariants
    // -----------------------------------------------------------------------

    #[test]
    fn test_blob_is_content_addressable() {
        let dir = setup("content_addressable");

        let code = "read_csv('data.csv')";
        let expected_hash = hash_string(code);

        add_input(&dir, code, Some("test")).expect("add input");

        // Blob should be stored at its content hash
        let blob_path = dtr_path(&dir).join("blobs").join(&expected_hash);
        assert!(blob_path.exists(), "blob should exist at content hash path");
        let content = fs::read_to_string(blob_path).expect("read blob file");
        assert_eq!(content, code);
    }

    // -----------------------------------------------------------------------
    // Clear-cache tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_clear_cache_clears_current_node() {
        let dir = setup("clear_cache_current");

        let hash = add_input(&dir, "read_csv('data.csv')", None).expect("add input");

        // Set a cache on the current node
        fs::create_dir_all(dtr_path(&dir).join("cache")).expect("mkdir");
        fs::write(dtr_path(&dir).join("cache").join("aaa"), "rds").expect("write");
        node_cache_set(&dir, &hash, "aaa").expect("set cache");

        // Clear cache
        clear_cache(&dir, false, false, false).expect("clear cache");

        // Node cache should be None
        let node = read_node(&dir, &hash).expect("read node");
        assert!(node.cache.is_none(), "cache should be cleared");

        // Cache file should still exist (only field cleared, not file -- the
        // file is content-addressed and might be shared)
        assert!(dtr_path(&dir).join("cache").join("aaa").exists(), "cache file remains");
    }

    #[test]
    fn test_clear_cache_noop_if_no_cache() {
        let dir = setup("clear_cache_noop");

        let hash = add_input(&dir, "read_csv('data.csv')", None).expect("add input");

        // No cache set — should succeed without error
        clear_cache(&dir, false, false, false).expect("clear cache with no cache");

        let node = read_node(&dir, &hash).expect("read node");
        assert!(node.cache.is_none(), "cache should still be none");
    }

    #[test]
    fn test_clear_cache_errors_without_cwn() {
        let dir = setup("clear_cache_no_cwn");

        let result = clear_cache(&dir, false, false, false);
        assert!(result.is_err(), "clear-cache with empty CWN should error");
        match result {
            Err(DtrError::NoCurrentNode) => {}
            _ => panic!("expected NoCurrentNode error"),
        }
    }

    #[test]
    fn test_clear_cache_errors_on_nonexistent_node() {
        let dir = setup("clear_cache_bad_node");

        cwn_write(&dir, "deadbeef").expect("write bad cwn");
        let result = clear_cache(&dir, false, false, false);
        assert!(result.is_err(), "clear-cache with bad CWN should error");
        match result {
            Err(DtrError::NodeNotFound(_)) => {}
            _ => panic!("expected NodeNotFound error"),
        }
    }

    #[test]
    fn test_clear_cache_recurse_children() {
        let dir = setup("clear_cache_children");

        let input = add_input(&dir, "read_csv('data.csv')", None).expect("input");
        let process = add(&dir, "process", "filter(x > 0)", None).expect("process");
        let chart = add(&dir, "chart", "ggplot()", None).expect("chart");

        // Cache process and chart
        fs::create_dir_all(dtr_path(&dir).join("cache")).expect("mkdir");
        fs::write(dtr_path(&dir).join("cache").join("bbb"), "rds").expect("write");
        fs::write(dtr_path(&dir).join("cache").join("ccc"), "rds").expect("write");
        node_cache_set(&dir, &process, "bbb").expect("cache process");
        node_cache_set(&dir, &chart, "ccc").expect("cache chart");

        // Clear from process with -rc
        cwn_write(&dir, &process).expect("goto process");
        clear_cache(&dir, true, false, false).expect("clear cache -rc");

        // Process and chart caches should be cleared
        assert!(read_node(&dir, &process).unwrap().cache.is_none(), "process cache cleared");
        assert!(read_node(&dir, &chart).unwrap().cache.is_none(), "chart cache cleared");

        // Input was never cached, unaffected
        assert!(read_node(&dir, &input).unwrap().cache.is_none(), "input unchanged");
    }

    #[test]
    fn test_clear_cache_recurse_parents() {
        let dir = setup("clear_cache_parents");

        let input = add_input(&dir, "read_csv('data.csv')", None).expect("input");
        let process = add(&dir, "process", "filter(x > 0)", None).expect("process");
        let chart = add(&dir, "chart", "ggplot()", None).expect("chart");

        // Cache input and process
        fs::create_dir_all(dtr_path(&dir).join("cache")).expect("mkdir");
        fs::write(dtr_path(&dir).join("cache").join("aaa"), "rds").expect("write");
        fs::write(dtr_path(&dir).join("cache").join("bbb"), "rds").expect("write");
        node_cache_set(&dir, &input, "aaa").expect("cache input");
        node_cache_set(&dir, &process, "bbb").expect("cache process");

        // Clear from chart with -rp
        clear_cache(&dir, false, true, false).expect("clear cache -rp");

        // All three should be cleared
        assert!(read_node(&dir, &input).unwrap().cache.is_none(), "input cache cleared");
        assert!(read_node(&dir, &process).unwrap().cache.is_none(), "process cache cleared");
        assert!(read_node(&dir, &chart).unwrap().cache.is_none(), "chart cache cleared");
    }

    #[test]
    fn test_clear_cache_all_clears_everything() {
        let dir = setup("clear_cache_all");

        let a = add_input(&dir, "read_csv('a.csv')", None).expect("a");
        let b = add_input(&dir, "read_csv('b.csv')", None).expect("b");
        add_merge(&dir, "inner_join(input_2, by='id', None)", &[&a, &b], None).expect("merge");

        // Cache all three
        fs::create_dir_all(dtr_path(&dir).join("cache")).expect("mkdir");
        fs::write(dtr_path(&dir).join("cache").join("x1"), "r1").expect("write");
        fs::write(dtr_path(&dir).join("cache").join("x2"), "r2").expect("write");
        fs::write(dtr_path(&dir).join("cache").join("x3"), "r3").expect("write");
        node_cache_set(&dir, &a, "x1").expect("cache a");
        node_cache_set(&dir, &b, "x2").expect("cache b");
        node_cache_set(&dir, &read_cwn(&dir).unwrap(), "x3").expect("cache merge");

        // Clear all
        clear_cache(&dir, false, false, true).expect("clear cache --all");

        // All node cache fields should be None
        let nodes_dir = dtr_path(&dir).join("nodes");
        for entry in fs::read_dir(nodes_dir).expect("read nodes dir") {
            let entry = entry.expect("entry");
            let content = fs::read_to_string(entry.path()).expect("read node");
            let node: Node = serde_json::from_str(&content).expect("parse node");
            assert!(node.cache.is_none(), "all node caches should be cleared");
        }

        // All cache files should be removed
        let cache_dir = dtr_path(&dir).join("cache");
        let cache_entries: Vec<_> = fs::read_dir(&cache_dir)
            .expect("read cache dir")
            .collect();
        assert!(cache_entries.is_empty(), "cache directory should be empty");
    }

    #[test]
    fn test_clear_cache_all_on_empty_project() {
        let dir = setup("clear_cache_all_empty");

        // No nodes at all — --all should succeed (no-op)
        clear_cache(&dir, false, false, true).expect("clear cache --all on empty project");

        let cache_dir = dtr_path(&dir).join("cache");
        assert!(cache_dir.exists(), "cache dir still exists");
        let entries: Vec<_> = fs::read_dir(&cache_dir).expect("read cache").collect();
        assert!(entries.is_empty(), "cache dir should be empty");
    }

    #[test]
    fn test_clear_cache_rc_and_rp_clears_both_directions() {
        let dir = setup("clear_cache_both");

        let input = add_input(&dir, "read_csv('data.csv')", None).expect("input");
        let process = add(&dir, "process", "filter(x > 0)", None).expect("process");
        let chart = add(&dir, "chart", "ggplot()", None).expect("chart");

        // Cache all three
        fs::create_dir_all(dtr_path(&dir).join("cache")).expect("mkdir");
        fs::write(dtr_path(&dir).join("cache").join("a"), "r").expect("write");
        fs::write(dtr_path(&dir).join("cache").join("b"), "r").expect("write");
        fs::write(dtr_path(&dir).join("cache").join("c"), "r").expect("write");
        node_cache_set(&dir, &input, "a").expect("cache input");
        node_cache_set(&dir, &process, "b").expect("cache process");
        node_cache_set(&dir, &chart, "c").expect("cache chart");

        // Clear from process with both -rc and -rp
        cwn_write(&dir, &process).expect("goto process");
        clear_cache(&dir, true, true, false).expect("clear cache -rc -rp");

        // All three should be cleared
        assert!(read_node(&dir, &input).unwrap().cache.is_none(), "input cleared");
        assert!(read_node(&dir, &process).unwrap().cache.is_none(), "process cleared");
        assert!(read_node(&dir, &chart).unwrap().cache.is_none(), "chart cleared");
    }

    #[test]
    fn test_clear_cache_root_node_no_parent() {
        let dir = setup("clear_cache_root");

        let input = add_input(&dir, "read_csv('data.csv')", None).expect("input");

        fs::create_dir_all(dtr_path(&dir).join("cache")).expect("mkdir");
        fs::write(dtr_path(&dir).join("cache").join("r"), "rds").expect("write");
        node_cache_set(&dir, &input, "r").expect("cache");

        // Clear with -rp on root node (no parents) — should succeed
        clear_cache(&dir, false, true, false).expect("clear cache -rp on root");

        assert!(read_node(&dir, &input).unwrap().cache.is_none(), "root cache cleared");
    }

    // -----------------------------------------------------------------------
    // Map tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_map_all_nodes() {
        let dir = setup("map_all");

        let a = add_input(&dir, "read_csv('a.csv')", Some("alpha")).expect("a");
        let b = add_input(&dir, "read_csv('b.csv')", Some("beta")).expect("b");
        let _process = add(&dir, "process", "filter(x > 0)", None).expect("process");

        let json = map(&dir, false, false).expect("map all");
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");

        let nodes = parsed["nodes"].as_object().expect("nodes object");
        assert_eq!(nodes.len(), 3, "should have 3 nodes");
        assert!(nodes.contains_key(&a), "should contain node a");
        assert!(nodes.contains_key(&b), "should contain node b");

        // Check marker names
        assert_eq!(nodes[&a]["marker"], "alpha");
        assert_eq!(nodes[&b]["marker"], "beta");

        // Check node types
        assert_eq!(nodes[&a]["node_type"], "input");

        // Check parent/child relationships
        let process_id = read_cwn(&dir).unwrap();
        let process_node = &nodes[&process_id];
        assert_eq!(process_node["parents"].as_array().unwrap().len(), 1);
        assert!(process_node["children"].as_array().unwrap().is_empty());
    }

    #[test]
    fn test_map_empty_project() {
        let dir = setup("map_empty");

        let json = map(&dir, false, false).expect("map empty");
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");

        let nodes = parsed["nodes"].as_object().expect("nodes object");
        assert!(nodes.is_empty(), "should have no nodes");
    }

    #[test]
    fn test_map_recurse_children() {
        let dir = setup("map_rc");

        let input = add_input(&dir, "read_csv('data.csv')", None).expect("input");
        let process = add(&dir, "process", "filter(x > 0)", None).expect("process");
        let chart = add(&dir, "chart", "ggplot()", None).expect("chart");

        // Branch from input to create a sibling subtree
        cwn_write(&dir, &input).expect("goto input");
        let model = add(&dir, "model", "glm(y ~ x)", None).expect("model");

        // Put CWN at process. -rc should give process + chart (descendants
        // of process), but NOT input (ancestor) or model (sibling branch).
        cwn_write(&dir, &process).expect("goto process");

        let json = map(&dir, true, false).expect("map -rc");
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");
        let nodes = parsed["nodes"].as_object().expect("nodes object");

        assert!(nodes.contains_key(&process), "should contain process");
        assert!(nodes.contains_key(&chart), "should contain chart");
        assert!(!nodes.contains_key(&input), "should NOT contain input (ancestor)");
        assert!(!nodes.contains_key(&model), "should NOT contain model (sibling branch)");
    }

    #[test]
    fn test_map_recurse_parents() {
        let dir = setup("map_rp");

        let input = add_input(&dir, "read_csv('data.csv')", None).expect("input");
        let process = add(&dir, "process", "filter(x > 0)", None).expect("process");
        let chart = add(&dir, "chart", "ggplot()", None).expect("chart");

        // Branch from process
        cwn_write(&dir, &process).expect("goto process");
        let model = add(&dir, "model", "glm(y ~ x)", None).expect("model");

        // CWN is at model. Map -rp should give model, process, input
        // but NOT chart.
        let json = map(&dir, false, true).expect("map -rp");
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");
        let nodes = parsed["nodes"].as_object().expect("nodes object");

        assert!(nodes.contains_key(&input), "should contain input");
        assert!(nodes.contains_key(&process), "should contain process");
        assert!(nodes.contains_key(&model), "should contain model");
        assert!(!nodes.contains_key(&chart), "should NOT contain chart (sibling)");
    }

    #[test]
    fn test_map_errors_without_cwn_for_rc() {
        let dir = setup("map_rc_no_cwn");

        let result = map(&dir, true, false);
        assert!(result.is_err(), "map -rc with empty CWN should error");
        match result {
            Err(DtrError::NoCurrentNode) => {}
            _ => panic!("expected NoCurrentNode error"),
        }
    }

    #[test]
    fn test_map_errors_without_cwn_for_rp() {
        let dir = setup("map_rp_no_cwn");

        let result = map(&dir, false, true);
        assert!(result.is_err(), "map -rp with empty CWN should error");
        match result {
            Err(DtrError::NoCurrentNode) => {}
            _ => panic!("expected NoCurrentNode error"),
        }
    }

    #[test]
    fn test_map_errors_on_nonexistent_node() {
        let dir = setup("map_bad_node");

        cwn_write(&dir, "deadbeef").expect("write bad cwn");
        let result = map(&dir, true, false);
        assert!(result.is_err(), "map with bad CWN should error");
        match result {
            Err(DtrError::NodeNotFound(_)) => {}
            _ => panic!("expected NodeNotFound error"),
        }
    }

    #[test]
    fn test_map_merge_node_includes_multiple_parents() {
        let dir = setup("map_merge");

        let left = add_input(&dir, "read_csv('left.csv')", Some("left")).expect("left");
        let right = add_input(&dir, "read_csv('right.csv')", Some("right")).expect("right");
        add_merge(&dir, "inner_join(right, by='id', None)", &[&left, &right], None).expect("merge");
        let merge_hash = read_cwn(&dir).unwrap();

        let json = map(&dir, false, false).expect("map all");
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");
        let nodes = parsed["nodes"].as_object().expect("nodes object");

        let merge_node = &nodes[&merge_hash];
        let parents = merge_node["parents"].as_array().unwrap();
        assert_eq!(parents.len(), 2);
        assert!(parents.contains(&serde_json::Value::String(left.clone())));
        assert!(parents.contains(&serde_json::Value::String(right.clone())));
    }

    #[test]
    fn test_map_includes_blob_and_cache() {
        let dir = setup("map_fields");

        let code = "read_csv('data.csv')";
        let hash = add_input(&dir, code, None).expect("add input");

        // Set a cache
        fs::create_dir_all(dtr_path(&dir).join("cache")).expect("mkdir");
        fs::write(dtr_path(&dir).join("cache").join("xxx"), "rds").expect("write");
        node_cache_set(&dir, &hash, "xxx").expect("set cache");

        let json = map(&dir, false, false).expect("map");
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");
        let node = &parsed["nodes"][&hash];

        assert_eq!(node["blob"], hash_string(code), "should include blob hash");
        assert_eq!(node["cache"], "xxx", "should include cache hash");
    }

    #[test]
    fn test_map_all_includes_nodes_from_disconnected_branches() {
        let dir = setup("map_all_disconnected");

        // Create two independent input nodes (no shared ancestry)
        let a = add_input(&dir, "read_csv('a.csv')", Some("first")).expect("a");
        let b = add_input(&dir, "read_csv('b.csv')", Some("second")).expect("b");

        // Default map should include both
        let json = map(&dir, false, false).expect("map all");
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");
        let nodes = parsed["nodes"].as_object().expect("nodes object");
        assert_eq!(nodes.len(), 2);
        assert!(nodes.contains_key(&a));
        assert!(nodes.contains_key(&b));
    }

    #[test]
    fn test_node_file_exists_and_is_json() {
        let dir = setup("node_file_json");

        let hash = add_input(&dir, "read_csv('data.csv')", None).expect("add input");
        let node_path = dtr_path(&dir).join("nodes").join(&hash);

        assert!(node_path.exists(), "node file should exist");
        let content = fs::read_to_string(node_path).expect("read node file");
        let parsed: serde_json::Value = serde_json::from_str(&content).expect("node should be valid JSON");
        assert_eq!(parsed["node_type"], "input");
        assert_eq!(parsed["blob"], hash_string("read_csv('data.csv')"));
    }
}
