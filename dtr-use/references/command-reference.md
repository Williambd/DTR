# DTR Command Reference

## `dtr init`

Initialize a DTR project in the current directory. Creates the `.dtr/` backend
directory with all required files: `CWN`, `markers`, `packages`,
`next_id`, `blobs/`, `nodes/`, and `cache/`.

```
dtr init
```

No flags.

---

## `dtr add input`

Add an input node (reads data). Input nodes are root nodes with no parent — they
read data from files or databases. Can be created even when CWN is empty. An
auto-generated marker (`input_N`) is created unless `-m` is given.

```
dtr add input [OPTIONS] <CODE>
```

| Flag | Description |
|------|-------------|
| `-m <MARKER>` | Optional marker name (auto-generated if omitted) |

**Examples:**
```bash
dtr add input "read_csv('sales.csv')" -m sales
dtr add input "readRDS('customers.rds')" -m cust
```

---

## `dtr add process`

Add a process node (dplyr transform: tibble → tibble). For `mutate()`,
`filter()`, `select()`, `arrange()`, `summarise()`, etc. Expects a tibble piped
via `|>` and produces a tibble. Requires a current working node.

```
dtr add process <CODE>
```

No flags.

**Examples:**
```bash
dtr add process "filter(amount > 100)"
dtr add process "mutate(month = month(date))"
```

---

## `dtr add chart`

Add a chart node (ggplot2 visualization). Expects a tibble piped via `|>` and
produces a ggplot object. Requires a current working node.

```
dtr add chart <CODE>
```

No flags.

**Examples:**
```bash
dtr add chart "ggplot(aes(month, amount)) + geom_col()"
dtr add chart "ggplot(aes(x, y)) + geom_point() + geom_smooth(method = 'lm')"
```

---

## `dtr add model`

Add a model node (statistical model fitting). For `glm()`, `lm()`, etc. Expects a
tibble piped via `|>` and produces a model object. Requires a current working node.

```
dtr add model <CODE>
```

No flags.

**Examples:**
```bash
dtr add model "glm(amount ~ month, family = poisson)"
dtr add model "lm(y ~ x1 + x2)"
```

---

## `dtr add merge`

Add a merge node (joins multiple parent outputs). The first parent's output pipes
via `|>`. Other parents are assigned to variables named after their markers (or
`p_<id>` fallback). Requires at least 2 parents.

```
dtr add merge <CODE> [PARENTS]...
```

No flags.

**Examples:**
```bash
dtr add merge "left_join(cust, by = 'id')" sales_hash cust
dtr add merge "inner_join(products, by = 'product_id')" sales_hash products
```

---

## `dtr add-lib`

Register an R package dependency for the project. Packages are imported via
`library()` at the top of every composed or executed R script. Duplicate
additions are ignored.

```
dtr add-lib <PACKAGE>
```

No flags.

**Examples:**
```bash
dtr add-lib dplyr
dtr add-lib ggplot2
dtr add-lib tidyr
dtr add-lib lubridate
```

---

## `dtr add-marker`

Give the current node a human-readable nickname. Markers can be used in place of
node IDs with `dtr goto`, `dtr add merge`, and `dtr compose` (as variable names).

```
dtr add-marker <NAME>
```

No flags.

**Examples:**
```bash
dtr add-marker sales_clean
dtr add-marker final_chart
dtr add-marker revenue_model
```

---

## `dtr read`

Print the R code of the current node to stdout.

```
dtr read
```

No flags.

---

## `dtr write`

Replace the R code of the current node. Invalidates the node's cache and all
descendant caches.

```
dtr write <CODE>
```

No flags.

**Examples:**
```bash
dtr write "filter(amount > 500, !is.na(date))"
dtr write "mutate(month = month(date), year = year(date))"
```

---

## `dtr goto`

Move to a different working node. Accepts a node ID, marker name, `".."`
(parent), or `"."` (child). Shortcuts error if there are multiple parents or
children — use an explicit ID or marker to disambiguate.

```
dtr goto <TARGET>
```

No flags.

**Examples:**
```bash
dtr goto ..            # Go to parent
dtr goto .             # Go to child (errors if multiple)
dtr goto sales         # Go by marker name
dtr goto n3            # Go by node ID
```

---

## `dtr delete`

Delete the current working node. Without `-r`, refuses to delete nodes that have
children. With `-r`, recursively deletes the node and all descendants. Moves CWN
to the first parent (or clears it for root nodes). Cleans up orphaned cache
files.

```
dtr delete [OPTIONS]
```

| Flag | Description |
|------|-------------|
| `-r` | Recursively delete this node and all descendants |

---

## `dtr compose`

Produce a runnable R script from the current node and all ancestors. Performs a
depth-first search upward through parents and emits a top-down pipe chain using
R's native `|>` operator. Package `library()` calls are prepended automatically.

```
dtr compose
```

No flags.

---

## `dtr run`

Execute the current node and print the result. Without `-r`, skips
recomputation of cached ancestors (uses their stored RDS output via
`readRDS()`). With `-r`, forces full recomputation from scratch.
Caches the result as RDS for future reuse and `dtr preview`.

```
dtr run [OPTIONS]
```

| Flag | Description |
|------|-------------|
| `-r` | Force full recomputation, ignore all caches |

---

## `dtr preview`

Preview the cached output of a node. Loads the cached RDS object and
auto-detects how to display it. Defaults to the current node unless a node ID
or marker is given.

```
dtr preview [TARGET]
```

| Object type | Preview behavior |
|-------------|-----------------|
| tibble / data.frame | Text preview (head 20 rows) |
| ggplot | Raw PNG bytes written to stdout |
| lm / glm | Model summary |
| other | `print()` output |

No flags.

**Examples:**
```bash
dtr preview              # Preview current node
dtr preview > plot.png   # Pipe ggplot PNG to file
dtr preview sales        # Preview a specific node by marker
dtr preview n3           # Preview by node ID
```

---

## `dtr cache`

Cache the current node's RDS output. Runs the composed script, saves the result
as RDS, hashes it, stores it in `.dtr/cache/`, and updates the node's cache
field. The next `dtr run` (without `-r`) will use this cache.

```
dtr cache
```

No flags.

---

## `dtr map`

Print a JSON description of the DAG. By default, returns all nodes with their
IDs, types, marker names, parents, children, blob hash, and cache hash.

```
dtr map [OPTIONS]
```

| Flag | Description |
|------|-------------|
| `-c`, `--recurse-children` | Current node + descendants only |
| `-p`, `--recurse-parents` | Current node + ancestors only |

---

## `dtr clear-cache`

Clear cached output of the current node.

```
dtr clear-cache [OPTIONS]
```

| Flag | Description |
|------|-------------|
| `-c`, `--recurse-children` | Clear all descendant caches |
| `-p`, `--recurse-parents` | Clear all ancestor caches |
| `--all` | Clear the entire cache for all nodes |
