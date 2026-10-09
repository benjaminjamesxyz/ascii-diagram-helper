use ascii_diagram::{BoxStyle, render_dsl_colored};
use clap::{Parser, Subcommand, ValueEnum};
use serde_json::Value;
use std::ffi::OsStr;
use std::fs;
use std::io::{self, IsTerminal, Read, Write};
use std::path::{Path, PathBuf};

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
    /// Honor diagram styling; otherwise use TTY detection and non-empty `NO_COLOR`
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
    ///
    /// Repeated flags are last-wins (e.g. `--style ascii --style heavy`).
    #[arg(
        short,
        long,
        value_enum,
        default_value = "rounded",
        global = true,
        overrides_with = "style"
    )]
    style: CliStyle,

    /// Wrap rendered output in a markdown fenced code block
    #[arg(short, long, global = true, overrides_with = "markdown")]
    markdown: bool,

    /// Node/edge emphasis colors: auto (diagram styling or TTY detection), always, never
    #[arg(
        long,
        value_enum,
        default_value = "auto",
        global = true,
        overrides_with = "color"
    )]
    color: ColorMode,

    /// Direct input string or file path (if no subcommand)
    ///
    /// If the input's first word is a subcommand name (render, dsl, example,
    /// help), prefix it with `--` (e.g. `ascii-diagram -- render`). Tokens after
    /// `--` are input, including tokens that look like flags.
    #[arg(num_args = 0..)]
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
        /// Type of example: flowchart, sequence, architecture, tree, table, stack, datastructure, doublylinkedlist, queue, heap, graph
        ///
        /// Aliases: ds, btree, bst, binarytree, linkedlist, array
        #[arg(value_name = "TYPE", default_value = "flowchart")]
        diagram_type: String,
    },
}

const VALID_EXAMPLE_TYPES: &str = "flowchart, sequence, architecture, tree, table, stack, datastructure, doublylinkedlist, queue, heap, graph";
const EXAMPLE_ALIASES: &str = "ds, btree, bst, binarytree, linkedlist, array";

const FLOWCHART_EXAMPLE: &str = r"graph TD
Client[Web App] -->|HTTPS| CDN[Cloudflare CDN]
CDN --> Gateway[API Gateway]
Gateway --> Auth[Auth Service]
Gateway --> Orders[Order Service]
Orders --> DB[(PostgreSQL)]";

/// Resolves an `example <TYPE>` argument to its spec. Matching is
/// case-insensitive; unknown types error with the full valid list — never a
/// silent fallback render.
fn example_spec(diagram_type: &str) -> Result<&'static str, String> {
    match diagram_type.to_lowercase().as_str() {
        "flowchart" => Ok(FLOWCHART_EXAMPLE),
        "sequence" => Ok(r"sequenceDiagram
participant Client
participant Gateway as API Gateway
participant Auth as Auth Service
participant DB as PostgreSQL
Client -> Gateway: POST /login
Gateway -> Auth: Authenticate
Auth -> DB: SELECT * FROM users
DB --> Auth: user_record
Auth --> Gateway: 200 OK (JWT)
Gateway --> Client: token"),
        "architecture" => Ok(
            r#"{"type":"architecture","containers":[{"id":"c1","title":"Production Cluster","layout":"row","items":[{"id":"api","name":"API Gateway","properties":[["Port","80"],["Protocol","HTTP"]]},{"id":"db","name":"Database","properties":[["Port","5432"],["Engine","Postgres"]]}]}],"connections":[{"from":"api","to":"db","label":"SQL"}]}"#,
        ),
        "tree" => Ok(r"src/
  main.rs (CLI entrypoint)
  canvas.rs (2D cell grid)
  flowchart.rs (Sugiyama layout)
  sequence.rs (Lifelines)
  parser.rs (Mermaid DSL)
Cargo.toml"),
        "stack" => Ok(r"stack
0xFFFF: Kernel Space
0xC000: User Stack (grows down)
Shared Libraries
Heap (grows up)
BSS Segment
0x0000: Code / Text"),
        "datastructure" | "ds" | "btree" => Ok(
            r#"{"type":"datastructure","kind":"btree","title":"B-Tree of order 4","btree_root":{"keys":["10","20"],"children":[{"keys":["3","5"]},{"keys":["12","15"]},{"keys":["25","30","35"]}]}}"#,
        ),
        "table" => Ok(r"table
color: cyan
| Service | Port | Protocol | Status |
| :--- | :---: | :---: | ---: |
| API Gateway | 8080 | HTTP | active |
| Auth Service | 8081 | HTTP | active |
| Database | 5432 | TCP | replica |"),
        "bst" | "binarytree" => Ok(
            r#"{"type":"datastructure","kind":"tree","title":"BST","values":["8","3","10","1","6","14","4"]}"#,
        ),
        "queue" => Ok(
            r#"{"type":"datastructure","kind":"queue","title":"Job Queue","nodes":["build","test","deploy"]}"#,
        ),
        "heap" => Ok(
            r#"{"type":"datastructure","kind":"heap","title":"Min-Heap","values":[1,3,2,6,4,5]}"#,
        ),
        "graph" => Ok(
            r#"{"type":"datastructure","kind":"graph","title":"Adjacency List","nodes":["a","b","c","d"],"edges":[["a","b"],["a","c"],["b","d"],["c","d"],["d","a"],["d","d"]]}"#,
        ),
        "linkedlist" => Ok(
            r#"{"type":"datastructure","kind":"linkedlist","title":"Linked List","nodes":["10","20","30"]}"#,
        ),
        "doublylinkedlist" => Ok(
            r#"{"type":"datastructure","kind":"doublylinkedlist","title":"Doubly Linked List","nodes":[10,20,30]}"#,
        ),
        "array" => {
            Ok(r#"{"type":"datastructure","kind":"array","title":"Array","values":["a","b","c"]}"#)
        }
        _ => Err(format!(
            "unknown example type '{diagram_type}' — valid types: {VALID_EXAMPLE_TYPES}; aliases: {EXAMPLE_ALIASES}"
        )),
    }
}

/// Colors for this render: explicit flag modes win over the environment;
/// auto mode enables color if the diagram explicitly requests it (e.g.
/// `classDef`, `stroke:`, `fill:`, `linkStyle`, `@color`, or JSON `color:`)
/// or if stdout is a terminal without `NO_COLOR`.
fn color_enabled(
    mode: ColorMode,
    no_color: Option<&OsStr>,
    tty: bool,
    has_explicit_color: bool,
) -> bool {
    match mode {
        ColorMode::Always => true,
        ColorMode::Never => false,
        ColorMode::Auto => {
            if has_explicit_color {
                true
            } else {
                tty && !no_color.is_some_and(|v| !v.is_empty())
            }
        }
    }
}

/// Returns true if the input diagram contains explicit color styling directives.
fn has_explicit_color_directives(input: &str) -> bool {
    input.contains("classDef")
        || input.contains("stroke:")
        || input.contains("fill:")
        || input.contains("linkStyle")
        || input.contains("\"color\":")
        || input.contains("color:")
        || input.lines().any(|l| {
            let t = l.trim();
            t.contains("@red")
                || t.contains("@green")
                || t.contains("@yellow")
                || t.contains("@blue")
                || t.contains("@magenta")
                || t.contains("@cyan")
                || t.contains("@orange")
                || t.contains("@purple")
        })
}

/// Unescape literal `\n` / `\t` at intake (the `dsl` reference behavior, now
/// applied to every user input mode for parity). JSON-shaped content is
/// exempt: JSON strings carry their own native `\n` escaping, which serde
/// decodes — a blind replace would turn legal `"a\nb"` into a raw control
/// character inside the JSON string and break parsing.
fn intake_unescape(input: &str) -> String {
    if input
        .trim_start()
        .trim_start_matches('\u{feff}')
        .starts_with('{')
    {
        return input.to_string();
    }
    input.replace("\\n", "\n").replace("\\t", "\t")
}

/// A JSON value that parses but is not an object (`[]`, `[1,2]`, `null`) can
/// never be a diagram spec — reject it up front instead of letting it fall
/// through to the DSL text fallback and echo back verbatim. Objects (`{...}`)
/// stay the parser's job; scalars/other text stay valid single-node DSL.
fn reject_non_object_json(input: &str) -> Result<(), String> {
    let trimmed = input.trim().trim_start_matches('\u{feff}').trim();
    if !(trimmed.starts_with('[') || trimmed == "null") {
        return Ok(());
    }
    match serde_json::from_str::<Value>(trimmed) {
        Ok(Value::Array(_)) => Err(
            "Invalid JSON diagram specification: JSON array input — expected a diagram object starting with '{' (e.g. {\"type\":\"flowchart\",...}) or a DSL header such as `graph TD`"
                .to_string(),
        ),
        Ok(Value::Null) => Err(
            "Invalid JSON diagram specification: JSON null input — expected a diagram object starting with '{' or a DSL header such as `graph TD`"
                .to_string(),
        ),
        _ => Ok(()), // not actually JSON — let the DSL parsers judge it
    }
}

/// Shared intake validation + render for every input mode. All input errors
/// surface as `Diagram Error: ...` with a non-zero exit at the call site.
fn render_input(input: &str, style: BoxStyle, colored: bool) -> Result<String, String> {
    if let Some(offset) = input.find('\0') {
        return Err(format!(
            "Input contains a NUL byte (U+0000) at byte offset {offset}; NUL is not valid in diagram input"
        ));
    }
    reject_non_object_json(input)?;
    render_dsl_colored(input, style, colored)
}

/// Render path for user-supplied text (all modes except the internal
/// `example` specs): intake-unescaped, then validated and rendered.
fn render_user_input(input: &str, style: BoxStyle, colored: bool) -> Result<String, String> {
    let unescaped = intake_unescape(input);
    render_input(&unescaped, style, colored)
}

/// Reads a diagram spec file. Bare mode and `render <file>` share this so
/// missing-file / directory cases report identical wording in both modes.
fn read_input_file(path: &Path) -> Result<String, String> {
    fs::read_to_string(path).map_err(|e| format!("Error reading file {}: {e}", path.display()))
}

fn read_stdin() -> Result<String, String> {
    let mut buffer = String::new();
    io::stdin()
        .read_to_string(&mut buffer)
        .map_err(|e| format!("Error reading stdin: {e}"))?;
    Ok(buffer)
}

enum BareInput {
    File(String),
    Inline(String),
}

/// Bare (no-subcommand) intake: an existing path (file or directory) is read
/// exactly like `render <file>` so error wording matches; a path-looking
/// single token that does not exist is a typo'd path, not DSL; everything
/// else is inline DSL. JSON specs and multi-word DSL are exempt from the
/// path-looking rule: JSON freely contains '/' inside values (e.g. "/users").
fn bare_input(args: &[String]) -> Result<BareInput, String> {
    if args.is_empty() {
        return read_stdin().map(BareInput::File);
    }
    let joined = args.join(" ");
    let path_like =
        joined.contains('/') && !joined.contains(' ') && !joined.trim_start().starts_with('{');
    if Path::new(&joined).exists() || path_like {
        read_input_file(Path::new(&joined)).map(BareInput::File)
    } else {
        Ok(BareInput::Inline(joined))
    }
}

/// Backtick run at the start of `line` after up to three spaces of
/// indentation — i.e. how long this line could act as a CommonMark fence.
fn fence_backticks(line: &str) -> usize {
    let indent = line.len() - line.trim_start_matches(' ').len();
    let line = if indent <= 3 {
        &line[indent..]
    } else {
        return 0;
    };
    line.chars().take_while(|&c| c == '`').count()
}

/// Wraps `rendered` in a CommonMark fenced code block. The fence escalates
/// past any backtick run at a line start in the content (§4.5): a content
/// line of N backticks would otherwise close an N-or-shorter fence mid-block.
fn markdown_code_block(rendered: &str) -> String {
    let longest = rendered.lines().map(fence_backticks).max().unwrap_or(0);
    let fence = "`".repeat(longest.max(2) + 1);
    format!("{fence}text\n{rendered}\n{fence}\n")
}

fn write_output_to<W: Write>(w: &mut W, rendered: &str, markdown: bool) -> io::Result<()> {
    let text = if markdown {
        markdown_code_block(rendered)
    } else {
        format!("{rendered}\n")
    };
    w.write_all(text.as_bytes())?;
    w.flush()
}

/// Emits the final output, converting stdout write failures (full disk,
/// closed pipe target) into the standard error path instead of a panic.
fn write_output(rendered: &str, markdown: bool) {
    let mut stdout = io::stdout().lock();
    if let Err(e) = write_output_to(&mut stdout, rendered, markdown) {
        fail(&format!("Error writing output: {e}"));
    }
}

fn fail(msg: &str) -> ! {
    eprintln!("{msg}");
    std::process::exit(1);
}

#[allow(
    clippy::too_many_lines,
    reason = "CLI dispatch across four input modes"
)]
fn main() {
    let cli = Cli::parse();
    let style: BoxStyle = cli.style.into();
    let (content, is_example) = match cli.command {
        Some(Commands::Dsl { dsl }) => (Ok(dsl), false),
        Some(Commands::Render { path }) => {
            let res = match path {
                Some(p) if p.to_str() != Some("-") => read_input_file(&p),
                _ => read_stdin(),
            };
            (res, false)
        }
        Some(Commands::Example { diagram_type }) => match example_spec(&diagram_type) {
            Ok(spec) => (Ok(spec.to_string()), true),
            Err(msg) => fail(&format!("Diagram Error: {msg}")),
        },
        None => match bare_input(&cli.input) {
            Ok(BareInput::File(content)) => (Ok(content), false),
            Ok(BareInput::Inline(dsl)) => (Ok(dsl), false),
            Err(msg) => fail(&msg),
        },
    };

    let content = match content {
        Ok(c) => c,
        Err(msg) => fail(&msg),
    };

    let has_explicit = has_explicit_color_directives(&content);
    let colored = color_enabled(
        cli.color,
        std::env::var_os("NO_COLOR").as_deref(),
        io::stdout().is_terminal(),
        has_explicit,
    );

    let output = if is_example {
        render_input(&content, style, colored)
    } else {
        render_user_input(&content, style, colored)
    };

    match output {
        Ok(rendered) => write_output(&rendered, cli.markdown),
        Err(err) => fail(&format!("Diagram Error: {err}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // OUT-01: no-color.org — empty NO_COLOR is ignored; any non-empty value
    // disables; explicit --color always/never override everything.
    #[test]
    fn auto_color_ignores_empty_no_color() {
        assert!(color_enabled(ColorMode::Auto, None, true, false));
        assert!(color_enabled(
            ColorMode::Auto,
            Some(OsStr::new("")),
            true,
            false
        ));
        assert!(!color_enabled(
            ColorMode::Auto,
            Some(OsStr::new("0")),
            true,
            false
        ));
        assert!(!color_enabled(
            ColorMode::Auto,
            Some(OsStr::new("1")),
            true,
            false
        ));
        assert!(!color_enabled(ColorMode::Auto, None, false, false));
    }

    #[test]
    fn explicit_color_mode_beats_no_color() {
        assert!(color_enabled(
            ColorMode::Always,
            Some(OsStr::new("1")),
            false,
            false
        ));
        assert!(!color_enabled(ColorMode::Never, None, true, false));
    }

    #[test]
    fn explicit_style_in_input_beats_pipe_and_no_color() {
        assert!(color_enabled(
            ColorMode::Auto,
            Some(OsStr::new("1")),
            false,
            true
        ));
        assert!(!color_enabled(
            ColorMode::Never,
            Some(OsStr::new("1")),
            false,
            true
        ));
    }

    // OUT-03: a content line of ``` must not close the block — escalate.
    #[test]
    fn markdown_escapes_fence_when_content_has_closing_fence() {
        let block = markdown_code_block("```\n  child");
        assert!(block.starts_with("````text\n"), "{block}");
        assert_eq!(block.lines().last(), Some("````"), "{block}");
        let inner = block.strip_prefix("````text\n").unwrap();
        assert!(inner.starts_with("```\n"), "{block}");
    }

    #[test]
    fn markdown_escapes_past_longest_backtick_run() {
        let block = markdown_code_block("ok\n``````\nok");
        assert!(block.starts_with("```````text\n"), "{block}");
    }

    #[test]
    fn fence_detection_ignores_non_fence_lines() {
        assert_eq!(fence_backticks("```"), 3);
        assert_eq!(fence_backticks("  ```"), 3); // ≤3 spaces can close
        assert_eq!(fence_backticks("    ```"), 0); // indented chunk, cannot
        assert_eq!(fence_backticks("```python"), 3);
        assert_eq!(fence_backticks("│ ``` │"), 0); // table cell, not line start
        assert_eq!(fence_backticks("dir ` quoted"), 0);
        assert_eq!(fence_backticks(""), 0);
    }

    // ERR-01: stdout write failure surfaces as an error, never a panic.
    struct AlwaysFails;
    impl Write for AlwaysFails {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            Err(io::Error::other("No space left on device"))
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn stdout_write_failure_returns_error_not_panic() {
        let mut w = AlwaysFails;
        assert!(write_output_to(&mut w, "A --> B", false).is_err());
        assert!(write_output_to(&mut w, "A --> B", true).is_err());
    }

    // ERR-02: NUL bytes are rejected up front, naming the offset.
    #[test]
    fn nul_byte_rejected_with_offset() {
        let err = render_input("a\0b", BoxStyle::Rounded, false).unwrap_err();
        assert!(err.contains("NUL"), "{err}");
        assert!(err.contains("offset 1"), "{err}");
        assert!(err.contains("U+0000"), "{err}");
    }

    // ARCH-E-02: JSON arrays/null error instead of echoing verbatim.
    #[test]
    fn json_array_and_null_rejected_not_echoed() {
        for bad in [
            "[]",
            "[1,2]",
            "null",
            "null\n",
            "  []",
            " null \n",
            "\u{feff}null",
        ] {
            let err = render_input(bad, BoxStyle::Rounded, false).unwrap_err();
            assert!(
                err.contains("Invalid JSON diagram specification"),
                "{bad}: {err}"
            );
        }
        let err = render_input("[]", BoxStyle::Rounded, false).unwrap_err();
        assert!(err.contains("array"), "{err}");
        let err = render_input("null", BoxStyle::Rounded, false).unwrap_err();
        assert!(err.contains("null"), "{err}");
    }

    #[test]
    fn non_json_and_object_input_unaffected() {
        // Only actual JSON is rejected; DSL that merely looks JSON-ish and
        // JSON objects keep their existing handling.
        assert!(reject_non_object_json("5").is_ok());
        assert!(reject_non_object_json("nullify").is_ok());
        assert!(reject_non_object_json("[unterminated").is_ok());
        assert!(reject_non_object_json("graph TD").is_ok());
        let err = render_input("{\"a\":1}", BoxStyle::Rounded, false).unwrap_err();
        assert!(err.contains("missing field `type`"), "{err}");
    }

    // IO-01: every user intake unescapes literal \n / \t like dsl mode.
    #[test]
    fn intake_unescape_matches_dsl_reference_behavior() {
        let via_escape =
            render_user_input(r"graph TD\n A --> B", BoxStyle::Rounded, false).unwrap();
        let direct = render_input("graph TD\n A --> B", BoxStyle::Rounded, false).unwrap();
        assert_eq!(via_escape, direct);
        assert_eq!(intake_unescape(r"a\tb"), "a\tb");
        assert_eq!(intake_unescape("plain"), "plain");
    }

    #[test]
    fn intake_unescape_leaves_json_strings_to_serde() {
        let json = r#"{"type":"datastructure","kind":"array","title":"a\nb","values":["x"]}"#;
        assert_eq!(intake_unescape(json), json);
        // The escaped form stays valid JSON and renders (title decodes via serde).
        assert!(render_user_input(json, BoxStyle::Rounded, false).is_ok());
    }

    // CLI-02 / DOC-04: unknown example type errors with the full type list.
    #[test]
    fn example_unknown_type_errors_with_type_list() {
        let err = example_spec("seqence").unwrap_err();
        assert!(err.contains("'seqence'"), "{err}");
        for t in [
            "flowchart",
            "sequence",
            "architecture",
            "tree",
            "table",
            "stack",
            "datastructure",
            "queue",
            "heap",
            "graph",
        ] {
            assert!(err.contains(t), "missing {t}: {err}");
        }
        for a in ["ds", "btree", "bst", "binarytree", "linkedlist", "array"] {
            assert!(err.contains(a), "missing alias {a}: {err}");
        }
    }

    #[test]
    fn example_type_matching_is_case_insensitive() {
        assert!(example_spec("FLOWCHART").is_ok());
        assert!(example_spec("DS").is_ok());
        assert!(example_spec("LinkedList").is_ok());
        assert!(example_spec("BinaryTree").is_ok());
    }

    // CLI-08: repeated flags are last-wins, not a clap usage error.
    #[test]
    fn repeated_style_flag_is_last_wins() {
        let cli = Cli::try_parse_from([
            "ascii-diagram",
            "dsl",
            "graph TD; A --> B",
            "--style",
            "ascii",
            "--style",
            "heavy",
        ])
        .unwrap();
        assert!(matches!(cli.style, CliStyle::Heavy));
    }

    #[test]
    fn repeated_style_across_subcommand_context_is_last_wins() {
        let cli = Cli::try_parse_from([
            "ascii-diagram",
            "--style",
            "ascii",
            "dsl",
            "graph TD",
            "--style",
            "heavy",
        ])
        .unwrap();
        assert!(matches!(cli.style, CliStyle::Heavy));
    }

    #[test]
    fn repeated_markdown_flag_is_accepted() {
        let cli = Cli::try_parse_from([
            "ascii-diagram",
            "--markdown",
            "--markdown",
            "dsl",
            "graph TD",
        ])
        .unwrap();
        assert!(cli.markdown);
    }

    #[test]
    fn repeated_color_flag_is_last_wins() {
        let cli = Cli::try_parse_from([
            "ascii-diagram",
            "--color",
            "never",
            "--color",
            "always",
            "dsl",
            "graph TD",
        ])
        .unwrap();
        assert!(matches!(cli.color, ColorMode::Always));
    }

    #[test]
    fn bare_input_parses_trailing_consumer_flags() {
        for input in [
            &["/tmp/spec.mmd"][..],
            &["graph TD; A --> B"],
            &["graph TD; A[--style] --> B"],
            &["graph", "TD;", "A[Start]"],
        ] {
            for (style, markdown) in [("-s", "-m"), ("--style", "--markdown")] {
                let cli = Cli::try_parse_from(
                    ["ascii-diagram"]
                        .into_iter()
                        .chain(input.iter().copied())
                        .chain([style, "double", markdown, "--color", "never"]),
                )
                .unwrap();
                assert!(cli.command.is_none());
                assert_eq!(cli.input, input);
                assert!(matches!(cli.style, CliStyle::Double));
                assert!(cli.markdown);
                assert!(matches!(cli.color, ColorMode::Never));
            }
        }
    }

    #[test]
    fn bare_input_rejects_incomplete_or_unknown_trailing_flags() {
        for input in ["/tmp/spec.mmd", "graph TD; A --> B"] {
            for flag in ["-s", "--style", "--color", "--unknown"] {
                let err = Cli::try_parse_from(["ascii-diagram", input, flag]).unwrap_err();
                assert_eq!(err.exit_code(), 2);
            }
            for (flag, next_flag) in [("--style", "--markdown"), ("--color", "-m")] {
                let err =
                    Cli::try_parse_from(["ascii-diagram", input, flag, next_flag]).unwrap_err();
                assert_eq!(err.exit_code(), 2);
            }
        }
    }

    #[test]
    fn end_of_options_keeps_flag_tokens_as_bare_input() {
        let input = [
            "graph", "TD;", "A", "-->", "B", "-s", "ascii", "-m", "--color", "never",
        ];
        let cli = Cli::try_parse_from(
            [
                "ascii-diagram",
                "--style",
                "heavy",
                "--color",
                "always",
                "--",
            ]
            .into_iter()
            .chain(input),
        )
        .unwrap();
        assert!(cli.command.is_none());
        assert_eq!(cli.input, input);
        assert!(matches!(cli.style, CliStyle::Heavy));
        assert!(!cli.markdown);
        assert!(matches!(cli.color, ColorMode::Always));
    }

    #[test]
    fn end_of_options_keeps_subcommand_names_as_bare_input() {
        for input in ["render", "dsl", "example", "help"] {
            let cli = Cli::try_parse_from(["ascii-diagram", "--", input, "--markdown"]).unwrap();
            assert!(cli.command.is_none());
            assert_eq!(cli.input, [input, "--markdown"]);
            assert!(!cli.markdown);
        }
    }
}
