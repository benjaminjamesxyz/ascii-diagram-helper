pub mod architecture;
pub mod canvas;
pub mod color;
pub mod datastructure;
pub mod flowchart;
pub mod parser;
pub mod schema;
pub mod sequence;
pub mod stack;
pub mod table;
pub mod theme;
pub mod tree;

use architecture::ArchitectureRenderer;
use datastructure::DataStructureRenderer;
use flowchart::FlowchartRenderer;
pub use parser::parse_dsl_or_json;
pub use schema::DiagramSpec;
use sequence::SequenceRenderer;
use stack::StackRenderer;
use table::TableRenderer;
pub use theme::{BoxStyle, Theme};
use tree::TreeRenderer;

#[must_use]
pub fn render_diagram(spec: &DiagramSpec) -> String {
    render_diagram_colored(spec, false).unwrap_or_else(|e| format!("Error: {e}"))
}

/// Renders with cell emphasis colors as ANSI SGR codes (colored variant).
///
/// # Errors
///
/// Returns `Err` when the spec is structurally incomplete (e.g. a
/// `datastructure` diagram without a root).
pub fn render_diagram_colored(spec: &DiagramSpec, colored: bool) -> Result<String, String> {
    match spec {
        DiagramSpec::Flowchart(f) => {
            let theme = Theme::new(f.style);
            let renderer = FlowchartRenderer::new(f, theme);
            Ok(renderer.render(colored))
        }
        DiagramSpec::Sequence(s) => {
            let theme = Theme::new(s.style);
            let renderer = SequenceRenderer::new(s, theme);
            Ok(renderer.render(colored))
        }
        DiagramSpec::Architecture(a) => {
            let theme = Theme::new(a.style);
            let renderer = ArchitectureRenderer::new(a, theme);
            Ok(renderer.render(colored))
        }
        DiagramSpec::Tree(t) => {
            let theme = Theme::new(t.style);
            let renderer = TreeRenderer::new(t, theme);
            Ok(renderer.render(colored))
        }
        DiagramSpec::Table(tbl) => {
            let theme = Theme::new(tbl.style);
            let renderer = TableRenderer::new(tbl, theme);
            Ok(renderer.render(colored))
        }
        DiagramSpec::Stack(stk) => {
            let theme = Theme::new(stk.style);
            let renderer = StackRenderer::new(stk, theme);
            Ok(renderer.render(colored))
        }
        DiagramSpec::DataStructure(ds) => {
            let theme = Theme::new(ds.style);
            let renderer = DataStructureRenderer::new(ds, theme);
            renderer.render(colored)
        }
    }
}

/// Parses `dsl` and renders the diagram as an ASCII/Unicode string.
///
/// # Errors
///
/// Returns `Err` if the DSL cannot be parsed (see [`parse_dsl_or_json`]).
pub fn render_dsl(dsl: &str, style: BoxStyle) -> Result<String, String> {
    render_dsl_colored(dsl, style, false)
}

/// Parses `dsl` and renders with ANSI emphasis colors when `colored`.
///
/// # Errors
///
/// Returns `Err` if the DSL cannot be parsed (see [`parse_dsl_or_json`]).
pub fn render_dsl_colored(dsl: &str, style: BoxStyle, colored: bool) -> Result<String, String> {
    let spec = parse_dsl_or_json(dsl, style)?;
    render_diagram_colored(&spec, colored)
}
