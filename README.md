# DTR

**Directed Graph R Project Organizer** — a CLI tool that makes large R data
analysis projects manageable by breaking them into small, reusable nodes on a
directed acyclic graph.

```
read_csv('sales.csv')          read_csv('customers.csv')
        │                              │
        ├── filter(amount > 100)       │
        │        │                     │
        │   mutate(month = ...)        │
        │        │                     │
        └────────┼─── left_join(cust, by = 'id')
                 │
          ggplot(...) + geom_col()
                 │
             lm(amount ~ month)
```

Each node is a single tidyverse expression — one `filter`, one `mutate`, one
`ggplot` call. Nodes chain via R's native `|>` pipe. The backend uses
content-addressed storage inspired by git, so every intermediate result is
cached and reproducible.

## Why DTR?

R analysis scripts tend to grow into thousand-line monoliths. Auditing a single
chart means scrolling through layers of transformations. Trying a different
approach means duplicating the script or commenting out code.

DTR solves this by making each analysis step a standalone node:

- **Trace any output to its source** — `dtr compose` prints the exact pipeline that produced a result
- **Branch fearlessly** — fork at any node without duplicating or commenting out code
- **Cache everything** — intermediate results are saved as RDS, so re-running only recomputes what changed
- **LLM-friendly** — agents can extend or branch DTR graphs without touching existing nodes

## Quick Start

### Install

```bash
cargo install --path .
```

Requires **R ≥ 4.1** and **Rscript** on `PATH`.

### A complete example

```bash
# Start a project
mkdir sales-analysis && cd sales-analysis
dtr init
dtr add-lib dplyr
dtr add-lib ggplot2

# Load two datasets
dtr add input "read_csv('sales.csv')" -m sales
dtr add input "read_csv('customers.csv')" -m cust

# Clean and transform
dtr goto sales
dtr add process "filter(!is.na(amount), amount > 0)" -m clean
dtr add process "mutate(month = floor_date(date, 'month'))" -m monthly

# Merge with customer data
dtr add merge "left_join(cust, by = 'customer_id')" clean cust -m enriched

# Visualize
dtr add chart "ggplot(aes(month, amount)) + geom_col() + facet_wrap(~region)"
dtr run                     # Execute and cache
dtr preview > revenue.png   # Save plot to PNG

# Model
dtr add model "lm(log(amount) ~ month + region, data = _)"
dtr run
dtr preview                 # Print model summary

# See what got us here
dtr compose     # Full runnable R script
dtr map         # JSON graph of all nodes
```

## Command Reference

### Project Setup

| Command | Description |
|---------|-------------|
| `dtr init` | Create a new DTR project in the current directory |
| `dtr add-lib <pkg>` | Register an R package (auto-imported in all scripts) |

### Building Nodes

| Command | Description |
|---------|-------------|
| `dtr add input <code> [-m name]` | Root node — reads data |
| `dtr add process <code> [-m name]` | Transform — filter, mutate, select, summarise |
| `dtr add chart <code> [-m name]` | Visualize — ggplot2 |
| `dtr add model <code> [-m name]` | Model — glm, lm, etc. |
| `dtr add merge <code> <parent...> [-m name]` | Join outputs from multiple parents |

All `add` commands accept `-m` to nickname the new node.

### Navigation

| Command | Description |
|---------|-------------|
| `dtr goto <target>` | Move to a node (ID, marker, `..` for parent, `.` for child) |
| `dtr read` | Print the current node's R code |
| `dtr write <code>` | Replace the current node's code (mutates in-place) |
| `dtr add-marker <name>` | Nickname the current node |
| `dtr delete [-r]` | Delete node (`-r` to recursively delete children) |

### Execution & Preview

| Command | Description |
|---------|-------------|
| `dtr compose` | Print the full runnable R pipeline |
| `dtr run [-r]` | Execute and cache (`-r` forces full recomputation) |
| `dtr cache` | Cache the current node's output |
| `dtr preview [node]` | Preview cached output — auto-detects tibble/ggplot/model |

`dtr preview` outputs text for tibbles and models, raw PNG bytes for ggplots
(pipe to file: `dtr preview > plot.png`).

### Inspection & Maintenance

| Command | Description |
|---------|-------------|
| `dtr map [-c] [-p]` | JSON graph of all nodes (`-c` descendants, `-p` ancestors) |
| `dtr clear-cache [-c] [-p] [--all]` | Clear cache (`-c` children, `-p` parents, `--all` everything) |

## Node Types

| Type | Input | Output | Example R code |
|------|-------|--------|----------------|
| `input` | — | tibble | `read_csv('data.csv')` |
| `process` | tibble | tibble | `filter(x > 0)` |
| `chart` | tibble | ggplot | `ggplot(aes(x, y)) + geom_point()` |
| `model` | tibble | model | `glm(y ~ x, family = binomial)` |
| `merge` | 2+ parents | tibble | `inner_join(right, by = 'id')` |

## How It Works

```
.dtr/
├── CWN          # Current working node (like git HEAD)
├── next_id      # Counter for node IDs — n0, n1, n2, ...
├── markers      # Named references to nodes (JSON)
├── packages     # R libraries to import (JSON array)
├── blobs/        # R code, addressed by SHA1 hash (immutable)
├── nodes/        # Node metadata — type, parents, children, blob ref (mutable)
└── cache/        # Cached outputs as RDS files (content-addressed)
```

Nodes use incrementing hex IDs (`n0`, `n1`, …, `n9`, `na`, `nb`, …). Blobs
are content-addressed — identical R code shares a single blob file. Caches
are RDS binary, keyed by SHA1 of the output.

When you run `dtr run`, ancestors with valid caches are loaded via `readRDS()`
instead of being recomputed. `dtr write` mutates the node in-place and clears
downstream caches automatically.

## Building from Source

```bash
git clone git@github.com:Williambd/DTR.git
cd DTR
cargo build --release
cargo test        # 128 tests
cp target/release/dtr ~/.local/bin/
```
