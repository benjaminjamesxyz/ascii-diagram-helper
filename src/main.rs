use ascii_diagram::{BoxStyle, render_dsl_colored};
use clap::{Parser, Subcommand, ValueEnum};
use std::fs;
use std::io::{self, IsTerminal, Read};
use std::path::PathBuf;

#[derive(ValueEnum, Clone, Copy, Debug)]
enum CliStyle {
    Rounded,
    Sharp,
    Double,
    Heavy,
    Ascii,
}

impl From<CliStyle> for BoxStyle {
    fn from(s: CliStyle) -> Self {
        match s {
            CliStyle::Rounded => BoxStyle::Rounded,
            CliStyle::Sharp => BoxStyle::Sharp,
            CliStyle::Double => BoxStyle::Double,
            CliStyle::Heavy => BoxStyle::Heavy,
            CliStyle::Ascii => BoxStyle::Ascii,
        }
    }
}

#[derive(ValueEnum, Clone, Copy, Debug, Default)]
enum ColorMode {
    /// Colors on for a terminal, off when piped; `NO_COLOR` env forces off
    #[default]
    Auto,
    Always,
    Never,
}

#[derive(Parser, Debug)]
#[command(name = "ascii-diagram")]
#[command(about = "High-precision terminal ASCII and Unicode diagram generator for Pi AI agent")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Border style: rounded, sharp, double, heavy, ascii
    #[arg(short, long, value_enum, default_value = "rounded", global = true)]
    style: CliStyle,

    /// Wrap rendered output in a markdown fenced code block
    #[arg(short, long, global = true)]
    markdown: bool,

    /// Node/edge emphasis colors: auto (TTY detection), always, never
    #[arg(long, value_enum, default_value = "auto", global = true)]
    color: ColorMode,

    /// Direct input string or file path (if no subcommand)
    #[arg(trailing_var_arg = true)]
    input: Vec<String>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Render diagram from a file or stdin (JSON or Mermaid DSL)
    Render {
        /// File path to read from (reads stdin if omitted or "-")
        #[arg(value_name = "FILE")]
        path: Option<PathBuf>,
    },
    /// Render diagram directly from inline DSL string
    Dsl {
        /// Diagram DSL string (e.g. "graph TD\n A --> B")
        dsl: String,
    },
    /// Generate example diagram specs for quick reference
    Example {
        /// Type of example: flowchart, sequence, architecture, tree, table, stack, datastructure, queue, heap, graph
        #[arg(value_name = "TYPE", default_value = "flowchart")]
        diagram_type: String,
    },
}

#[allow(
    clippy::too_many_lines,
    reason = "CLI dispatch across four input modes"
)]
fn main() {
    let cli = Cli::parse();
    let style: BoxStyle = cli.style.into();
    let colored = match cli.color {
        ColorMode::Always => true,
        ColorMode::Never => false,
        ColorMode::Auto => std::env::var_os("NO_COLOR").is_none() && io::stdout().is_terminal(),
    };

    let output = match cli.command {
        Some(Commands::Dsl { dsl }) => {
            // The --help example advertises escaped sequences ('graph TD\n A --> B'):
            // unescape literal backslash-n / backslash-t at intake.
            let unescaped = dsl.replace("\\n", "\n").replace("\\t", "\t");
            render_dsl_colored(&unescaped, style, colored)
        }
        Some(Commands::Render { path }) => {
            let content = match path {
                Some(p) if p.to_str() != Some("-") => fs::read_to_string(&p).unwrap_or_else(|e| {
                    eprintln!("Error reading file {}: {}", p.display(), e);
                    std::process::exit(1);
                }),
                _ => {
                    let mut buffer = String::new();
                    io::stdin().read_to_string(&mut buffer).unwrap_or_else(|e| {
                        eprintln!("Error reading stdin: {e}");
                        std::process::exit(1);
                    });
                    buffer
                }
            };
            render_dsl_colored(&content, style, colored)
        }
        Some(Commands::Example { diagram_type }) => {
            let example_dsl = match diagram_type.to_lowercase().as_str() {
                "sequence" => {
                    r"sequenceDiagram
participant Client
participant Gateway as API Gateway
participant Auth as Auth Service
participant DB as PostgreSQL
Client -> Gateway: POST /login
Gateway -> Auth: Authenticate
Auth -> DB: SELECT * FROM users
DB --> Auth: user_record
Auth --> Gateway: 200 OK (JWT)
Gateway --> Client: token"
                }
                "architecture" => {
                    r#"{"type":"architecture","containers":[{"id":"c1","title":"Production Cluster","layout":"row","items":[{"id":"api","name":"API Gateway","properties":[["Port","80"],["Protocol","HTTP"]]},{"id":"db","name":"Database","properties":[["Port","5432"],["Engine","Postgres"]]}]}],"connections":[{"from":"api","to":"db","label":"SQL"}]}"#
                }
                "tree" => {
                    r"src/
  main.rs (CLI entrypoint)
  canvas.rs (2D cell grid)
  flowchart.rs (Sugiyama layout)
  sequence.rs (Lifelines)
  parser.rs (Mermaid DSL)
Cargo.toml"
                }
                "stack" => {
                    r"stack
0xFFFF: Kernel Space
0xC000: User Stack (grows down)
Shared Libraries
Heap (grows up)
BSS Segment
0x0000: Code / Text"
                }
                "datastructure" | "ds" | "btree" => {
                    r#"{"type":"datastructure","kind":"btree","title":"B-Tree of order 4","btree_root":{"keys":["10","20"],"children":[{"keys":["3","5"]},{"keys":["12","15"]},{"keys":["25","30","35"]}]}}"#
                }
                "table" => {
                    r"table
color: cyan
| Service | Port | Protocol | Status |
| :--- | :---: | :---: | ---: |
| API Gateway | 8080 | HTTP | active |
| Auth Service | 8081 | HTTP | active |
| Database | 5432 | TCP | replica |"
                }
                "bst" | "binarytree" => {
                    r#"{"type":"datastructure","kind":"tree","title":"BST","values":["8","3","10","1","6","14","4"]}"#
                }
                "queue" => {
                    r#"{"type":"datastructure","kind":"queue","title":"Job Queue","nodes":["build","test","deploy"]}"#
                }
                "heap" => {
                    r#"{"type":"datastructure","kind":"heap","title":"Min-Heap","values":[1,3,2,6,4,5]}"#
                }
                "graph" => {
                    r#"{"type":"datastructure","kind":"graph","title":"Adjacency List","nodes":["a","b","c","d"],"edges":[["a","b"],["a","c"],["b","d"],["c","d"],["d","a"],["d","d"]]}"#
                }
                _ => {
                    r"graph TD
Client[Web App] -->|HTTPS| CDN[Cloudflare CDN]
CDN --> Gateway[API Gateway]
Gateway --> Auth[Auth Service]
Gateway --> Orders[Order Service]
Orders --> DB[(PostgreSQL)]"
                }
            };
            render_dsl_colored(example_dsl, style, colored)
        }
        None => {
            // Check if trailing input provided
            if cli.input.is_empty() {
                // Try reading from stdin
                let mut buffer = String::new();
                io::stdin().read_to_string(&mut buffer).unwrap_or_else(|e| {
                    eprintln!("Error reading stdin: {e}");
                    std::process::exit(1);
                });
                render_dsl_colored(&buffer, style, colored)
            } else {
                let joined = cli.input.join(" ");
                // If it's a file path that exists, read it; a path-looking arg
                // (contains '/') that does NOT exist is almost certainly a typo
                // — error instead of silently rendering it as inline DSL.
                if std::path::Path::new(&joined).is_file() {
                    let content = fs::read_to_string(&joined).unwrap_or_else(|e| {
                        eprintln!("Error reading file {joined}: {e}");
                        std::process::exit(1);
                    });
                    render_dsl_colored(&content, style, colored)
                } else if joined.contains('/')
                    && !joined.contains(' ')
                    && !joined.trim_start().starts_with('{')
                {
                    // Path-looking single-token arg that does not exist.
                    // JSON specs and multi-word DSL are exempt: JSON freely
                    // contains '/' inside values (e.g. "/users" endpoints).
                    eprintln!("Error: file not found: {joined}");
                    std::process::exit(1);
                } else {
                    // Inline DSL; unescape literal \n / \t like `dsl` mode.
                    let unescaped = joined.replace("\\n", "\n").replace("\\t", "\t");
                    render_dsl_colored(&unescaped, style, colored)
                }
            }
        }
    };

    match output {
        Ok(rendered) => {
            if cli.markdown {
                println!("```text\n{rendered}\n```");
            } else {
                println!("{rendered}");
            }
        }
        Err(err) => {
            eprintln!("Diagram Error: {err}");
            std::process::exit(1);
        }
    }
}
