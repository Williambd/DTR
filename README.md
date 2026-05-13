# DTR — Directed Graph R Project Organizer

DTR atomizes R data analysis into nodes on a directed acyclic graph (DAG).
Each node contains a single R/tidyverse expression chained via R's native `|>`
pipe. The backend mirrors git internals — content-addressed blobs, mutable
node metadata, and named refs.

## Why DTR?

- **Explore forking paths** — branch analysis without bloated scripts or new notebooks
- **Reproducible outputs** — `dtr compose` produces the exact code needed to reach any node
- **Agentic LLM friendly** — agents can extend or branch DTR paths without altering existing scripts
- **Enforced tidyverse** — every node is a single dplyr/ggplot2 expression

## Quick Start

```bash
# Install R packages
Rscript -e 'install.packages(c("dplyr","ggplot2","readr"))'

# Create a project
dtr init
dtr add-lib dplyr
dtr add-lib ggplot2

# Load data
dtr add input "read_csv('sales.csv')" -m sales

# Build a pipeline
dtr add process "filter(amount > 100)"
dtr add process "mutate(month = month(date))"
dtr add chart "ggplot(aes(month, amount)) + geom_col()"

# Execute
dtr run          # Run with cached ancestors
dtr run -r       # Force full recomputation

# Preview results
dtr preview              # Text preview for tibbles
dtr preview > plot.png   # PNG output for ggplots

# Inspect
dtr compose     # Full runnable R script
dtr read        # Current node's R code
dtr map         # JSON description of the DAG
```

## Installation

```bash
cargo build --release
cp target/release/dtr ~/.local/bin/
```

Requires **R ≥ 4.1** (for native `|>` pipe) and **Rscript** on `PATH`.

## Commands

| Command | Description |
|---------|-------------|
| `dtr init` | Initialize `.dtr/` in the current directory |
| `dtr add input <code> [-m name]` | Add root node that reads data |
| `dtr add process <code> [-m name]` | Add dplyr transform (filter, mutate, select...) |
| `dtr add chart <code> [-m name]` | Add ggplot2 visualization |
| `dtr add model <code> [-m name]` | Add statistical model (glm, lm...) |
| `dtr add merge <code> <parent...> [-m name]` | Join outputs from multiple parents |
| `dtr read` | Print current node's R code |
| `dtr write <code>` | Replace current node's code (mutates in-place) |
| `dtr goto <target>` | Move CWN (`..` = parent, `.` = child, marker or ID) |
| `dtr delete [-r]` | Delete node (`-r` = recursive) |
| `dtr add-marker <name>` | Nickname the current node |
| `dtr add-lib <package>` | Register an R package dependency |
| `dtr compose` | Output full runnable R script |
| `dtr run [-r]` | Execute and cache (`-r` = force full recomputation) |
| `dtr cache` | Explicitly cache current node's output |
| `dtr preview [node]` | Preview cached output (auto-detects type) |
| `dtr map [-c] [-p]` | JSON DAG description |
| `dtr clear-cache [-c] [-p] [--all]` | Clear cached outputs |

## Node Types

| Type | Input | Output | Example |
|------|-------|--------|---------|
| `input` | — | tibble | `read_csv('data.csv')` |
| `process` | tibble | tibble | `filter(x > 0)` |
| `chart` | tibble | ggplot | `ggplot(aes(x,y)) + geom_point()` |
| `model` | tibble | model | `glm(y ~ x, family = binomial)` |
| `merge` | N parents | tibble | `inner_join(right, by = 'id')` |

## Backend (`.dtr/`)

```
.dtr/
├── CWN          # Current working node (like git HEAD)
├── next_id      # Incrementing counter for node IDs (n0, n1, n2...)
├── markers      # name → node ID (JSON)
├── packages     # R library dependencies (JSON array)
├── blobs/        # R code, content-addressed by SHA1 (immutable)
├── nodes/        # Node metadata as JSON (mutable in-place)
└── cache/        # Cached outputs as RDS (content-addressed by SHA1)
```

## Building from Source

```bash
git clone <repo>
cd DTR
cargo build --release
cargo test          # 128 tests
```

## License

[Your license here]
