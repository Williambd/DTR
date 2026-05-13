# DTR Development Guide for pi

DTR is a Rust CLI that organizes R data analysis as a directed acyclic graph.
Backend mirrors git: content-addressed blobs (SHA1), mutable node metadata (JSON),
markers (like git refs), and CWN (like HEAD).

## Development Rules

1. **Tests first** — write tests, then implement.
2. **Spec first** — `projectspec.txt` is the single source of truth. Re-read it for ambiguity.
3. **Build + test**: `cargo test` (128 tests, must pass).
4. **Rebuild after every change**:
   ```bash
   cargo build --release && cp target/release/dtr ~/.local/bin/dtr
   ```
5. **Blocker tracking**: append to `.pi-blockers.md` if stuck.

## Coding Style

- Simple, readable, concise. No over-engineering.
- Functional: pure functions, immutable data, iterators, pattern matching.
- Small focused functions. Descriptive but short variable names.
- Idiomatic Rust: `Result`/`Option`, `clap`, `serde`, `sha1`.

## Key Source Files

| File | Purpose |
|------|---------|
| `src/main.rs` | CLI definitions (clap), command dispatch |
| `src/lib.rs` | All library functions, internal helpers, 128 tests |
| `projectspec.txt` | Canonical specification |
| `dtr.1` | Manpage (groff) |
| `audit/` | Code audit reports |

## Architecture

### Backend Layout (`.dtr/`)

```
.dtr/
├── CWN          # Current working node ID (like git HEAD)
├── next_id      # Counter for node IDs (u64)
├── markers      # name → node ID (JSON object)
├── packages     # ["dplyr", "ggplot2"] (JSON array)
├── blobs/       # R code by SHA1 (immutable, content-addressed)
├── nodes/       # Node metadata JSON (mutable in place by ID)
└── cache/       # RDS outputs by SHA1
```

### Node Struct

```rust
pub struct Node {
    pub node_type: String,       // "input" | "process" | "chart" | "model" | "merge"
    pub parents: Vec<String>,    // parent node IDs
    pub parent_vars: Vec<String>, // variable names for each parent ("" = pipes via |>)
    pub children: Vec<String>,   // child node IDs
    pub blob: String,            // SHA1 hash of R code in blobs/
    pub cache: Option<String>,   // SHA1 hash of cached RDS output
}
```

### Key Design Decisions

1. **Nodes mutable, blobs immutable**: Node files updated in place by ID.
   Blobs are content-addressed — same code → same hash → deduplication.
2. **Node IDs**: incrementing hex with `n` prefix (`n0`, `n1`, …, `n9`, `na`, …).
3. **Cache**: unified RDS binary. `compose_node_cached` uses `readRDS()` for cached ancestors.
4. **Library imports**: `dtr add-lib <pkg>` stores in `.dtr/packages`. `run()`, `cache()`, and `compose()` auto-prepend `library()` calls.
5. **Marker resolution**: `resolve_ref()` tries marker name first, then node ID directly.
6. **`dtr write` mutates in-place**: updates blob reference on same node file. No ID change, no repointing, no orphans.

### compose_impl Pattern

```rust
fn compose_impl(dir, hash, recurse: fn(&Path, &str) -> Result<String, DtrError>) -> ...
fn compose_node(dir, hash) { compose_impl(dir, hash, compose_node) }
fn compose_node_cached(dir, hash) { /* cache check */ compose_impl(dir, hash, compose_node_cached) }
```

### DtrError

```rust
pub enum DtrError {
    Io(std::io::Error),
    NoCurrentNode,
    InvalidNodeType(String),
    NodeNotFound(String),
    InvalidState(String),
    RExecError(String),
    Json(serde_json::Error),
}
```

## Key Library Functions

| Function | Purpose |
|----------|---------|
| `init()` | Create `.dtr/` directory structure |
| `add()` / `add_input()` / `add_merge()` | Node creation |
| `add_marker()` / `add_lib()` | Project metadata |
| `read_current()` / `write_current()` | Inspect and edit |
| `goto()` | Navigation (`..`, `.`, marker, ID) |
| `delete_current()` | Deletion (recursive, cache cleanup) |
| `compose()` | Full R script generation |
| `compose_node()` / `compose_node_cached()` | Recursive composition |
| `run()` / `cache()` | Execute via Rscript |
| `preview()` | Preview cached RDS → `PreviewOutput::Text` or `Png` |
| `wrap_r_script()` | Wrap composed chain in braces + print + saveRDS |
| `map()` | Serialize the DAG as JSON |
| `clear_cache()` | Clear cache with `-c`/`-p`/`--all` |
| `clear_descendant_caches()` | Invalidate downstream caches |
| `execute_r_script()` | Shell out to Rscript |
| `resolve_ref()` | Marker name → node ID |
| `hash_string()` / `hash_bytes()` | SHA1 hashing |

## Edge Cases

- `dtr add input` works with empty CWN (root nodes).
- `dtr add merge` requires ≥2 parents. Accepts IDs and markers.
- `dtr goto ..` errors on multiple parents. `dtr goto .` errors on multiple children.
- `dtr delete` without `-r` errors if node has children.
- `dtr write` mutates in-place, clears descendant caches. Node ID stays the same.
- Merge variable names stored in `parent_vars` at creation time.
- R must be installed (`Rscript` on PATH) for `run`, `cache`, and `preview`.
- Temp files for Rscript use PID + nanosecond suffix for concurrency safety.
