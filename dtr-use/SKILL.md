---
name: dtr-use
description: >-
  Usage skill for the DTR CLI — a Rust tool that organizes R data analysis as a
  directed acyclic graph of nodes.
  TRIGGER: Whenever the user asks to do data analysis, work with R/tidyverse, or
  inspect/modify a pipeline, AND a `.dtr/` directory exists in the working
  directory (or the user mentions DTR), use this skill for all pipeline building,
  navigation, execution, preview, and debugging tasks.
  Covers all commands, flags, best-practice patterns, and anti-patterns.
---

# DTR-Use — Building R Analysis Pipelines with DTR

DTR atomizes R data analysis into nodes on a DAG. Each node holds a single
tidyverse expression chained via R's native `|>` pipe.

## Inspecting a Pipeline (for pi)

When asked to examine or describe a DTR pipeline, **always use DTR's built-in
introspection commands** — never read `.dtr/` internal files directly.

| Task | Command |
|------|---------|
| View the full DAG structure (JSON) | `dtr map` |
| View ancestors up to current node (JSON) | `dtr map -p` |
| View current node + descendants (JSON) | `dtr map -c` |
| See the runnable R script for the current node | `dtr compose` |
| Read the R code of the current node only | `dtr read` |
| Preview a cached node's output (chart/model/tibble) | `scripts/dtr-preview [target]` then `read` the printed path |

**Previewing cached output** — use the `scripts/dtr-preview` helper (relative
to this skill directory). It saves the output to a temp file and prints the
path with a `IMAGE:` or `TEXT:` prefix. After running it, use `read` on the
printed file path to display the result (images render natively, text shows
inline). Do NOT pipe `dtr preview` through bash — the raw binary won't display.

Example:
```bash
scripts/dtr-preview n4
# → IMAGE:/tmp/dtr-preview-output.png
read /tmp/dtr-preview-output.png
```

`dtr map` gives you node IDs, types, markers, parents, children, and cache
status in structured JSON. Use `dtr compose` to see the full pipeline as a
runnable R script with `library()` calls and `|>` chains. Use `dtr read` for
the current node's code alone. **Do not** read files under `.dtr/` — those are
an internal implementation detail and the format may change.

## Command Reference (summary)

For full details and examples, see `references/command-reference.md`.

| Command | Syntax | Flags | Description |
|---------|--------|-------|-------------|
| `init` | `dtr init` | — | Initialize `.dtr/` project directory |
| `add input` | `dtr add input <CODE>` | `-m <MARKER>` | Root node — reads data (CSV, RDS, etc.) |
| `add process` | `dtr add process <CODE>` | — | dplyr transform (tibble → tibble) |
| `add chart` | `dtr add chart <CODE>` | — | ggplot2 visualization (tibble → ggplot) |
| `add model` | `dtr add model <CODE>` | — | Statistical model (tibble → model) |
| `add merge` | `dtr add merge <CODE> <PARENTS>...` | — | Join ≥2 parent outputs |
| `add-lib` | `dtr add-lib <PACKAGE>` | — | Register R package dependency |
| `add-marker` | `dtr add-marker <NAME>` | — | Nickname the current node |
| `read` | `dtr read` | — | Print current node's R code |
| `write` | `dtr write <CODE>` | — | Replace code; invalidates downstream caches |
| `goto` | `dtr goto <TARGET>` | — | Navigate: node ID, marker, `..`, `.` |
| `delete` | `dtr delete` | `-r` | Delete node; `-r` for recursive |
| `compose` | `dtr compose` | — | Full runnable R script with `|>` chain |
| `run` | `dtr run` | `-r` | Execute; `-r` forces full recomputation |
| `preview` | `dtr preview [TARGET]` | — | Preview cached RDS (auto-detects type) |
| `cache` | `dtr cache` | — | Cache current node as RDS |
| `map` | `dtr map` | `-c`, `-p` | JSON DAG description; `-c` children, `-p` parents |
| `clear-cache` | `dtr clear-cache` | `-c`, `-p`, `--all` | Clear cached output |

## Patterns (Best Practices)

### Prefer Tidyverse and Modern R Syntax

Always use tidyverse verbs from dplyr, tidyr, ggplot2, etc. Prefer the native
`|>` pipe (R ≥ 4.1) over `%>%` from magrittr. Use modern idioms like
`across()`, `.by` grouping, `reframe()`, and `case_match()` where appropriate.
Avoid base-R idioms like `df[df$x > 0, ]` or `df$y <- df$x + 1` when a tidy
equivalent exists.

```r
# Good
filter(amount > 100) |>
  mutate(month = month(date))

# Avoid
df[df$amount > 100, ]
df$month <- month(df$date)
```

### One Atomic Operation Per Node

Each node should contain the smallest sensible unit of R code — typically one
dplyr verb or one ggplot addition. If you find yourself writing multiple steps
in a single expression, split them into separate nodes.

```r
# Good: three atomic nodes
# Node A: filter(amount > 100)
# Node B: mutate(month = month(date))
# Node C: summarise(total = sum(amount), .by = month)

# Avoid: one bloated node
# filter(amount > 100) |> mutate(month = month(date)) |> summarise(total = sum(amount), .by = month)
```

This gives you granular caching, easy reordering, and the ability to inspect
intermediate results at each step.

### Non-Input Nodes Receive Piped Input

All `process`, `chart`, `model`, and `merge` nodes automatically receive the
output of their parent piped via `|>`. Your R code should **not** reference or
re-read the parent's output — it's already the implicit first argument.

```r
# Correct: the tibble from the parent is already piped in
filter(amount > 100)

# Wrong: do not re-read or re-reference upstream data
filter(sales_data, amount > 100)
```

### Proper Merge Node Setup

When merging two datasets that are already loaded into nodes, use the `dtr add
merge` command with both parents specified. The **first parent's** output is
piped via `|>`. **Additional parents** are available as variables named after
their markers.

```bash
# Load two datasets
dtr add input "read_csv('sales.csv')" -m sales
dtr add input "read_csv('customers.csv')" -m cust

# Navigate back to the first dataset (or wherever the merge should attach)
dtr goto sales

# Create the merge node: sales pipes via |>, cust is available as variable 'cust'
dtr add merge "left_join(cust, by = 'customer_id')" sales cust
```

The merge expression uses `cust` as a variable because that marker was set on
the customer input node. If a parent has no marker, use its node ID with a `p_`
prefix (e.g., `p_n3`).

### Run `dtr compose` Regularly to Orient Yourself

Running `dtr compose` at any point shows the full pipeline from root to current
node as a runnable R script. Use this frequently — especially before deciding
what the next node should be — so you can see where you are in the pipeline and
what transformations have already been applied.

```bash
dtr compose   # Review the full pipeline so far
dtr read      # Review just the current node's code
dtr preview   # Preview the cached output of the current node
dtr map -p    # See the DAG structure leading to the current node
```

### Share Computations in Common Ancestors

When two or more branches need the same computation, place it in a single
shared ancestor node. Branch from there — never duplicate the calculation
across sibling nodes. DTR's DAG model naturally supports fan-out: one node can
have many children, each building on the same output.

```
# Good: single-source YOY calculation, two branches consume it
n3: mutate(cpi_pct_change = (cpi / lag(cpi, 12) - 1) * 100) ──┬── n4: chart (monthly Phillips curve)
                                                                │
                                                                └── na: mutate(year = year(date)) ── nb: summarise(..., .by = year) ── nc: chart (yearly averages)

# Avoid: duplicated YOY calculation in both siblings
n3: mutate(cpi_pct_change = ...) ── n4: chart
n6: mutate(year = year(date), cpi_pct_change = ...) ── n7 ── n8: chart
```

Benefits of shared ancestors:
- **Single source of truth** — change the formula in one place, both branches
  update
- **Deduplicated cache** — the computation runs once, not N times
- **Content-addressed blob dedup** — DTR stores identical R code once via
  SHA1, so identical chart nodes (same ggplot call) share the same blob
  automatically

When you find yourself writing the same expression in two nodes, ask: can
this live in a common ancestor that both branches inherit?

---

## Anti-Patterns (What to Avoid)

### Don't Write Changed Outputs to a New CSV

DTR manages caches of data changes automatically via RDS files in `.dtr/cache/`.
There is no need to write intermediate results back to CSV or other file
formats. Let DTR handle persistence.

```bash
# Wrong: writing intermediate results to disk manually
dtr add process "mutate(amount = amount * 1.1) |> write_csv('adjusted.csv')"

# Correct: let DTR cache the result
dtr add process "mutate(amount = amount * 1.1)"
dtr cache    # Explicitly cache if needed, or just dtr run
```

### Don't Pack Multi-Step Logic Into One Node

If a node contains multiple dplyr verbs, multiple ggplot layers that could stand
alone, or unrelated steps, split it. Monolithic nodes defeat DTR's DAG model:
they prevent granular caching, make debugging harder, and obscure the analysis
flow.

```r
# Wrong: too many unrelated steps in one node
filter(!is.na(date)) |>
  mutate(month = month(date), quarter = quarter(date)) |>
  group_by(quarter) |>
  summarise(mean_amt = mean(amount), total = sum(amount))

# Correct: atomic nodes
# Node 1: filter(!is.na(date))
# Node 2: mutate(month = month(date), quarter = quarter(date))
# Node 3: summarise(mean_amt = mean(amount), total = sum(amount), .by = quarter)
```

### Don't Name Nodes and Markers Poorly

Use descriptive marker names that reflect what the node produces, not how it
does it. Good markers describe the output schema or conceptual meaning of the
data at that point.

```bash
# Good
dtr add-marker sales_clean
dtr add-marker monthly_revenue
dtr add-marker customer_segments

# Avoid
dtr add-marker node3
dtr add-marker filtered
dtr add-marker temp
```

### Don't Ignore Cache Invalidation

After `dtr write`, remember that downstream caches are automatically
invalidated. Run `dtr run` (or `dtr run -r` if you want to force recomputation
from scratch) to regenerate results. Using stale assumptions about cached
results after an edit leads to confusion.

### Don't Use Non-Tidyverse Patterns

Avoid `$` indexing, `subset()`, `attach()`, base `plot()`, or other non-tidy
patterns. DTR is designed around the tidyverse. Mixing paradigms makes the
pipeline harder to reason about and compose.

```r
# Avoid
df$new_col <- df$x + df$y
plot(df$x, df$y)

# Use
mutate(new_col = x + y)
ggplot(aes(x, y)) + geom_point()
```

### Don't Duplicate Logic Across Sibling Nodes

If two nodes with the same parent contain the same computation, you've created
bloat. Move the shared logic into the common ancestor and branch from there.

```bash
# Wrong: n3 and n6 both compute cpi_pct_change, same parent (n2)
dtr goto n2
dtr add process "mutate(cpi_pct_change = (cpi / lag(cpi, 12) - 1) * 100)"   # n3
dtr goto n2
dtr add process "mutate(year = year(date), cpi_pct_change = (cpi / lag(cpi, 12) - 1) * 100)"  # n6 — duplicated!

# Right: n3 is the single source of truth; branch from n3 with only new logic
dtr goto n2
dtr add process "mutate(cpi_pct_change = (cpi / lag(cpi, 12) - 1) * 100)"   # n3
dtr add chart "ggplot(...)"                                                  # n4 (monthly)
dtr goto n3
dtr add process "mutate(year = year(date))"                                  # na — only new column
```

Duplication causes three problems:
1. **Bloat** — two nodes holding the same expression
2. **Fragile edits** — changing the formula requires updating both nodes
3. **Wasted recomputation** — the same calculation runs twice when caches are
   cold

Before adding a new node, always check: does a sibling already compute this?
If so, branch from that sibling instead.

### Don't Read `.dtr/` Internals Directly (for pi)

DTR provides purpose-built introspection commands: `dtr map`, `dtr compose`,
`dtr read`, and `dtr preview`. Always use these — never `cat` or `read` files
inside `.dtr/nodes/`, `.dtr/blobs/`, `.dtr/markers`, or `.dtr/CWN`. The
internal format is an implementation detail that may change. The CLI commands
are the stable interface and give you a complete, correct picture with one
invocation each.
