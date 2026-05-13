# DTR

**Directed Graph R Project Organizer** — a CLI tool that makes large R data
analysis projects manageable by breaking them into small, reusable nodes on a
directed acyclic graph.


 ```
   n0 [input · cpi] ───────────┐
                               ├── n2 [merge] ──┬── n3 [process] ─┬── n4 [chart · phillips_curve]        ✓
   n1 [input · unemployment] ──┘                │                 │
                                                │                 └── na [process] ── nc [chart · phillips_yearly] ★ ✓
                                                │
                                                └── n5 [chart · unemployment_trend]                       ✓
 ```

Each node is a single tidyverse expression — one `filter`, one `mutate`, one
`ggplot` call. Nodes chain via R's native `|>` pipe. The backend uses
content-addressed storage inspired by git, so every intermediate result is
cached and reproducible.

## Why DTR?

R analysis scripts tend to grow into large monoliths. Auditing a single
chart means scrolling through layers of transformations. Trying a different
approach means duplicating the script or commenting out code.

DTR solves this by making each analysis step a standalone node:

- **Trace any output to its source** — `dtr compose` prints the exact pipeline that produced a result
- **Branch fearlessly** — fork at any node without duplicating or commenting out code
- **Cache everything** — intermediate results are saved as RDS, so re-running only recomputes what changed
- **LLM-friendly** — Tool is designed with agentic use in mind, via the DTR-USE skill (seperate repo).

## Project Roadmap:
Basically I wanted to build something that is to R-markdown what R-markdown is to pure R (or jupyter notebook to a python script). Cell-based notebook analysis is a great tool for data analysts: you get to experiment, view intermediate outputs, try stuff out. It's more natural to use, while still keeping a record of what you're doing. You don't need to rerun intensive processes that are unchanged in your code

But the limitations are real too. Verifying the methodology of one chart can require reading through a large mono notebook of analysis. Often, larger teams break up their analysis & model building into multiple notebooks & pipelines, where intermediate outputs are stored and whose lineage is carefully tracked, to ensure downstream users are properly using. Code gets replicated across notebooks on a single project, so that analysts can branch and try multiple ideas flexibly.

A lot of this basically comes down to problems of functional programming. Analysts are generally writing functional code, but their meta-project (intermediate files, interactions between pipelines and models etc) and tools (like jupyter or RMD) are state-dependent.

DTR is trying to simplify this by conceiving of your analysis as a graph of inputs, processing decisions, visualizations, and models. In dtr, lineage tracing is automatic, you can always see EXACTLY what code creates the current node. Intermediate outputs are cached and automatically cleared by upstream changes, so there's never any need to load them seperately, or to make sure a collaborator has run the script already. You only need to store your primary data. Code is functional and replicable by default. Auditing is easy, because you can view the pipeline that made the chart/model you are auditing, without the noise of other aspects of the analysis.

Why R and not python? Basically R comes equipped with the functional programming toolkit needed for me to develop dtr with the free time I had. I would be extremely interested in building (or seeing) something that replicates the dtr architecture but for python, but I suspect inconsistencies between libraries would make this a much more involved project.

Pretty much all the actual code has been agentically written in rust, but the architecture has been carefully planned exclusively by a human (me) and is heavily inspired by the basic structure of git. My current workflow in bug catching is to work on personal projects using DTR, and then as bugs come up, deciding what solutions fit the long-term needs of the project. I'm also regularly running "Agent Code Audits" (see audits folder) that focus on catching implementation errors that could come back to haunt the project. 

An important part of DTR is the agent skill. While I think dtr is a great tool in general, I recognize that adding each node via shell isn't the most intuitive for humans. This can probably be fixed via an excellent gui, but that's just not the thing I'm personally most interested in building right now. The dtr-use skill repo is my attempt to build the optimal interface for doing analysis in dtr (I'm using pi-mono, but trying to keep it broadly compatible). (Side note: the economics of building a skill interface for a new tool are really strange. The cost of learning to use a new tool is basically being transfered from the user to the creator, which is a wierd tradeoff to experience.)

Anyways, give dtr a try! Reach out to me with any issues or features you want! Enjoy!

## Quick Start

Requires **Rust**, **R ≥ 4.1**, and **Rscript** on `PATH`.



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

## Install

```bash
git clone git@github.com:Williambd/DTR.git
cd DTR
cargo build --release
cp target/release/dtr ~/.local/bin/    # or anywhere on PATH
```

Run the test suite:

```bash
cargo test        # 128 tests
```
