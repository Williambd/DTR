use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "dtr",
    version,
    about = "A directed graph R project organizer",
    long_about = "DTR atomizes R data analysis pipelines into nodes on a directed \
acyclic graph. Each node contains a single R/tidyverse expression \
that chains via the native |> pipe. This makes large analysis \
projects organized, reproducible, and explorable.\n\n\
Nodes are stored in a .dtr directory (git-inspired backend). \
Blobs (R code) are content-addressed by SHA1. Nodes use \
incrementing hex IDs. CWN tracks the current working node.\n\n\
NODE TYPES:\n  \
  input    Read data (dplyr)\n  \
  process  Transform tibbles (dplyr::mutate, filter, select, etc.)\n  \
  chart    Visualize data (ggplot2)\n  \
  model    Fit statistical models (glm, lm, etc.)\n  \
  merge    Join outputs from multiple parents (dplyr joins)",
    after_help = "EXAMPLES:\n  \
  dtr init\n  \
  dtr add-lib dplyr\n  \
  dtr add-lib ggplot2\n  \
  dtr add input \"read_csv('sales.csv')\" -m sales\n  \
  dtr add process \"filter(amount > 100)\"\n  \
  dtr add chart \"ggplot(aes(date, amount)) + geom_line()\"\n  \
  dtr compose\n  \
  dtr run\n  \
  dtr run -r\n  \
  dtr cache\n  \
  dtr goto ..\n  \
  dtr goto sales\n  \
  dtr read\n  \
  dtr write \"filter(amount > 500)\"\n  \
  dtr delete -r\n  \
  dtr add-marker final_chart\n\
\nBACKEND (.dtr/):\n  \
  CWN       Current working node (like git HEAD)\n  \
  cwnout    Output of the last dtr run\n  \
  next_id   Incrementing counter for node IDs\n  \
  markers   Named references to nodes\n  \
  packages  R library dependencies (JSON array)\n  \
  blobs/    R code, content-addressed by SHA1\n  \
  nodes/    Node metadata (JSON)\n  \
  cache/    Cached outputs (RDS format)",
    after_long_help = "SEE ALSO:\n  R(1), Rscript(1)"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Initialize a DTR project in the current directory.
    ///
    /// Creates the .dtr/ backend directory with all required files
    /// and subdirectories: CWN, cwnout, markers, packages, next_id,
    /// blobs/, nodes/, and cache/.
    Init,

    /// Add a node to the current working node.
    ///
    /// Creates a child node of the given type and moves CWN to it.
    /// Input nodes can be created without a parent (root nodes).
    /// Merge nodes accept multiple parents by hash or marker name.
    /// Process, chart, and model nodes require a current working node.
    Add {
        #[command(subcommand)]
        kind: AddKind,
    },

    /// Give the current node a human-readable nickname.
    ///
    /// Markers can be used in place of node IDs with dtr goto,
    /// dtr add merge, and dtr compose (as variable names).
    AddMarker {
        /// Marker name (e.g. sales_data, final_chart)
        name: String,
    },

    /// Register an R package dependency for the project.
    ///
    /// Packages are imported via library() at the top of every
    /// composed or executed R script. Duplicate additions are ignored.
    AddLib {
        /// R package name (e.g. dplyr, ggplot2, tidyr)
        package: String,
    },

    /// Print the R code of the current node to stdout.
    Read,

    /// Replace the R code of the current node.
    ///
    /// Invalidates the node's cache and all descendant caches.
    Write {
        /// R code to write (replaces the current node's code)
        code: String,
    },

    /// Move to a different working node.
    ///
    /// Accepts a node ID, marker name, ".." (parent), or "." (child).
    /// Shortcuts error if there are multiple parents or children —
    /// use an explicit ID or marker to disambiguate.
    Goto {
        /// Node ID, marker name, ".." (parent), or "." (child)
        target: String,
    },

    /// Delete the current working node.
    ///
    /// Without -r, refuses to delete nodes that have children.
    /// With -r, recursively deletes the node and all descendants.
    /// Moves CWN to the first parent (or clears it for root nodes).
    /// Cleans up orphaned cache files.
    Delete {
        /// Recursively delete this node and all descendants
        #[arg(short = 'r')]
        recursive: bool,
    },

    /// Produce a runnable R script from the current node and all ancestors.
    ///
    /// Performs a depth-first search upward through parents and emits
    /// a top-down pipe chain using R's native |> operator. Package
    /// library() calls are prepended automatically.
    Compose,

    /// Execute the current node and print the result.
    ///
    /// Without -r, skips recomputation of cached ancestors (uses their
    /// stored RDS output via readRDS()). With -r, forces full
    /// recomputation from scratch. Output is also written to .dtr/cwnout.
    Run {
        /// Force full recomputation, ignore all caches
        #[arg(short = 'r')]
        force: bool,
    },

    /// Cache the current node's RDS output.
    ///
    /// Runs the composed script, saves the result as RDS, hashes it,
    /// stores it in .dtr/cache/, and updates the node's cache field.
    /// The next dtr run (without -r) will use this cache.
    Cache,
}

#[derive(Subcommand)]
enum AddKind {
    /// Add an input node (reads data).
    ///
    /// Input nodes have no parents — they are root nodes that read
    /// data from files or databases. Can be created even when CWN is
    /// empty. An auto-generated marker (input_N) is created unless
    /// -m is given.
    Input {
        /// R code that reads data (e.g. read_csv, readRDS, dbGetQuery)
        code: String,
        /// Optional marker name (auto-generated if omitted)
        #[arg(short = 'm')]
        marker: Option<String>,
    },

    /// Add a process node (dplyr transform: tibble to tibble).
    ///
    /// For mutate(), filter(), select(), arrange(), summarise(), etc.
    /// Expects a tibble piped via |> and produces a tibble.
    Process {
        /// R code for the dplyr transform
        code: String,
    },

    /// Add a chart node (ggplot2 visualization).
    ///
    /// Expects a tibble piped via |> and produces a ggplot object.
    Chart {
        /// R code for the chart
        code: String,
    },

    /// Add a model node (statistical model fitting).
    ///
    /// For glm(), lm(), etc. Expects a tibble piped via |>
    /// and produces a model object.
    Model {
        /// R code for the model
        code: String,
    },

    /// Add a merge node (joins multiple parent outputs).
    ///
    /// The first parent's output pipes via |>. Other parents are
    /// assigned to variables named after their markers (or p_<id>
    /// fallback). Requires at least 2 parents.
    Merge {
        /// R code for the merge/join
        code: String,
        /// Parent node IDs or marker names (at least 2)
        parents: Vec<String>,
    },
}

fn exec(dir: &std::path::Path) -> Result<(), dtr::DtrError> {
    match Cli::parse().command {
        Command::Init => dtr::init(dir),

        Command::Add { kind } => match kind {
            AddKind::Input { code, marker } => {
                let hash = dtr::add_input(dir, &code, marker.as_deref())?;
                println!("added input node {hash}");
                Ok(())
            }
            AddKind::Process { code } => {
                let hash = dtr::add(dir, "process", &code)?;
                println!("added process node {hash}");
                Ok(())
            }
            AddKind::Chart { code } => {
                let hash = dtr::add(dir, "chart", &code)?;
                println!("added chart node {hash}");
                Ok(())
            }
            AddKind::Model { code } => {
                let hash = dtr::add(dir, "model", &code)?;
                println!("added model node {hash}");
                Ok(())
            }
            AddKind::Merge { code, parents } => {
                let refs: Vec<&str> = parents.iter().map(|s| s.as_str()).collect();
                let hash = dtr::add_merge(dir, &code, &refs)?;
                println!("added merge node {hash}");
                Ok(())
            }
        },

        Command::AddMarker { name } => {
            let hash = dtr::add_marker(dir, &name)?;
            println!("marked node {hash} as '{name}'");
            Ok(())
        }
        Command::AddLib { package } => {
            dtr::add_lib(dir, &package)?;
            println!("added library {package}");
            Ok(())
        }
        Command::Read => {
            let code = dtr::read_current(dir)?;
            print!("{code}");
            Ok(())
        }
        Command::Write { code } => {
            let hash = dtr::write_current(dir, &code)?;
            println!("wrote code to node {hash}");
            Ok(())
        }
        Command::Goto { target } => {
            dtr::goto(dir, &target)?;
            Ok(())
        }
        Command::Delete { recursive } => {
            dtr::delete_current(dir, recursive)?;
            println!("deleted current node");
            Ok(())
        }
        Command::Compose => {
            let script = dtr::compose(dir)?;
            println!("{script}");
            Ok(())
        }
        Command::Cache => {
            let cache_hash = dtr::cache(dir)?;
            println!("cached output as {cache_hash}");
            Ok(())
        }
        Command::Run { force } => {
            let output = dtr::run(dir, force)?;
            print!("{output}");
            Ok(())
        }
    }
}

fn main() {
    let cwd = std::env::current_dir().expect("failed to get current directory");
    if let Err(e) = exec(&cwd) {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}
