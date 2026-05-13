---
name: dtr-dev
description: Development skill for the DTR project — a Rust CLI that organizes R data analysis as a directed acyclic graph of nodes. Use for implementing, modifying, testing, and debugging the DTR codebase itself.
---

# DTR — Directed Graph R Project Organizer

DTR atomizes R data analysis into nodes on a DAG. Each node holds a single
tidyverse expression chained via R's native `|>` pipe.

## Core Commands

### Project Setup

```bash
dtr init              # Initialize .dtr/ in current directory
dtr add-lib dplyr     # Register R package dependencies
dtr add-lib ggplot2   # (library() calls auto-prepended to scripts)
```

### Building a Pipeline

```bash
dtr add input "read_csv('sales.csv')" -m sales    # Root node (reads data)
dtr add process "filter(amount > 100)"             # Transform
dtr add process "mutate(month = month(date))"      # Another transform
dtr add chart "ggplot(aes(month, amount)) + geom_col()"  # Visualize
dtr add model "glm(amount ~ month, family = poisson)"    # Model
```

### Merging Branches

```bash
dtr add input "read_csv('customers.csv')" -m cust
dtr add merge "left_join(cust, by = 'id')" sales_processed_hash cust
```

### Navigation

```bash
dtr goto ..          # Parent node
dtr goto .           # Child node (errors if multiple)
dtr goto sales       # By marker name
dtr goto n3          # By node ID
```

### Inspecting and Editing

```bash
dtr read             # Print current node's R code
dtr write "filter(amount > 500)"  # Replace code (invalidates downstream caches)
dtr add-marker final_result       # Nickname the current node
dtr compose          # Output full runnable R script
```

### Execution and Caching

```bash
dtr run              # Execute (uses cached ancestors where available)
dtr run -r           # Force full recomputation (ignore cache)
dtr cache            # Explicitly cache current node's output as RDS
```

### Previewing Results

```bash
dtr preview          # Preview cached output of current node
#   <node>   Preview a specific node by ID or marker name
```

Auto-detects the object type:
- tibble / data.frame → text preview (head 20 rows)
- ggplot → raw PNG bytes written to stdout
- lm / glm → model summary
- other → print() output

### Cache Management

```bash
dtr clear-cache             # Clear current node's cache
#   -c   Clear all descendant caches
#   -p   Clear all ancestor caches
#   --all  Clear the entire cache directory
```

### Graph Inspection

```bash
dtr map              # JSON description of the entire DAG
#   -c   Current node + descendants only
#   -p   Current node + ancestors only
```

### Deletion

```bash
dtr delete           # Delete leaf node (errors if it has children)
dtr delete -r        # Recursively delete node and all descendants
```

## Node Types

| Type | Input | Output | Typical R code |
|------|-------|--------|---------------|
| `input` | — | tibble | `read_csv('data.csv')` |
| `process` | tibble | tibble | `filter(x > 0)`, `mutate(z = x + y)` |
| `chart` | tibble | ggplot | `ggplot(aes(x,y)) + geom_point()` |
| `model` | tibble | model | `glm(y ~ x, family = binomial)` |
| `merge` | N parents | tibble | `inner_join(right, by = 'id')` |

## Key Behaviors

- **Node IDs**: incrementing hex with `n` prefix (`n0`, `n1`, `n2`...).
- **CWN**: `.dtr/CWN` tracks the current working node (like git HEAD).
- **Content-addressed blobs**: same R code → same SHA1 hash → deduplicated.
- **Mutable nodes**: node files are mutated in place by ID.
- **Cache format**: RDS binary. `dtr run` and `dtr cache` both produce RDS.
- **Preview**: `dtr preview` reads cached RDS and auto-detects how to display it.
  Returns `PreviewOutput::Text` (tibbles/models) or `PreviewOutput::Png` (ggplots).
- **Cache invalidation**: `dtr write` clears the node's cache AND all descendant caches.
- **Library imports**: packages registered via `dtr add-lib` are auto-prepended.
- **Merge variables**: side-parents named by marker or `p_<id>` fallback at creation time.
- **Backward compatibility**: `parent_vars` field uses `#[serde(default)]`.

## Development Workflow for pi

When implementing or modifying DTR features:

1. **Tests first** — write tests that define expected behavior, then implement.
2. **Spec first** — consult `projectspec.txt` for requirements. If ambiguous, ask the user.
3. **Build + test**: `cargo test` (124 tests, must pass).
4. **Manpage**: update `dtr.1` when adding or changing commands.
5. **Knowledge context**: update `.pi-knowledge-context.md` for architectural changes.
6. **Blocker tracking**: log roadblocks to `.pi-blockers.md`.

### Key Source Files

| File | Purpose |
|------|---------|
| `src/main.rs` | CLI definitions (clap), command dispatch |
| `src/lib.rs` | All library functions, internal helpers, tests |
| `projectspec.txt` | Canonical specification |
| `dtr.1` | Manpage (groff) |
| `audit1.txt` | Code audit results and fix status |
| `.pi-knowledge-context.md` | Project knowledge for pi |

### Key Library Functions

| Function | Purpose |
|----------|---------|
| `init()` | Create `.dtr/` directory structure |
| `add()` / `add_input()` / `add_merge()` | Node creation |
| `add_lib()` / `add_marker()` | Project metadata |
| `read_current()` / `write_current()` | Inspect and edit |
| `goto()` | Navigation with `..` and `.` shortcuts |
| `delete_current()` | Deletion (recursive, cache cleanup) |
| `compose()` | Full R script generation |
| `run()` | Execute via Rscript (cache-aware) |
| `cache()` | Run and cache as RDS |
| `preview()` | Preview cached RDS → `PreviewOutput::Text` or `Png` |
| `build_preview_r_script()` | Generate type-detecting R preview script |
| `wrap_r_script()` | Wrap composed chain in braces + print + saveRDS |
| `compose_impl()` | Shared compose body (takes recurse fn pointer) |
| `compose_node()` / `compose_node_cached()` | Thin compose wrappers |
| `resolve_ref()` | Resolve marker name → node ID |
| `clear_descendant_caches()` | Invalidate downstream caches |
| `clear_cache()` | Clear cache with -c/-p/--all flags |
| `map()` | Serialize the DAG as JSON |
| `execute_r_script()` | Shell out to Rscript |

### Error Handling

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

### Backend Layout (`.dtr/`)

```
.dtr/
├── CWN          # Current working node ID
├── next_id      # Counter for node IDs (u64)
├── markers      # name → node ID (JSON object)
├── packages     # ["dplyr", "ggplot2"] (JSON array)
├── blobs/       # R code by SHA1 (immutable)
├── nodes/       # Node JSON (mutable in place)
└── cache/       # RDS outputs by SHA1
```

### Installation & Quick Rebuild

After any code change, rebuild and reinstall from the repo root:

```bash
cargo build --release && cp target/release/dtr ~/.local/bin/dtr
```

Pi should run this automatically after implementing any feature or bugfix. Verify with:

```bash
dtr --version
```

For the full first-time install (includes manpage):

```bash
cargo build --release
cp target/release/dtr ~/.local/bin/
cp dtr.1 ~/.local/share/man/man1/
```

## Edge Cases to Remember

- `dtr add input` works with empty CWN (root nodes need no parent).
- `dtr add merge` requires ≥2 parents. Accepts both node IDs and marker names.
- `dtr goto ..` errors on multiple parents (use explicit ID/marker).
- `dtr goto .` errors on multiple children (use explicit ID/marker).
- `dtr delete` without `-r` errors if node has children.
- `dtr write` creates a new node ID and updates all references. Clears downstream caches.
- `dtr compose` and `dtr run` auto-prepend `library()` calls from `packages`.
- Merge variable names stored in `parent_vars` at creation time for consistency.
- R must be installed for `dtr run` and `dtr cache` (`Rscript` must be on PATH).
- `Rscript` temp files use PID + nanosecond suffix to avoid parallel-test races.
