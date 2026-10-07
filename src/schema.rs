use crate::color::Color;
use crate::theme::BoxStyle;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "UPPERCASE")]
pub enum LayoutDirection {
    #[default]
    TB, // Top to Bottom
    TD, // Top Down (alias for TB)
    LR, // Left to Right
    BT, // Bottom to Top
    RL, // Right to Left
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum NodeShape {
    #[default]
    Box, // [Text]
    Rounded,          // (Text)
    Diamond,          // {Text} - Decision
    Database,         // [(Text)] - Cylinder
    Subprocess,       // [[Text]]
    Stadium,          // ([Text]) - Pill
    Circle,           // ((Text))
    Hexagon,          // {{Text}} - Preparation / condition
    DoubleCircle,     // (((Text))) - Start / end point
    Parallelogram,    // [/Text/] - Input
    ParallelogramAlt, // [\Text\] - Output (lean left)
    Trapezoid,        // [/Text\] - Manual input
    TrapezoidAlt,     // [\Text/] - Manual operation
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NodeSpec {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub shape: NodeShape,
    /// Dashed border, set via Mermaid `class`/`style` `stroke-dasharray`.
    #[serde(default)]
    pub dashed_border: bool,
    /// Border color, set via Mermaid `class`/`style` `stroke:<name|hex>`.
    /// Applied to border/line glyphs only — label text stays default.
    #[serde(default)]
    pub color: Option<Color>,
    /// Label text color, set via Mermaid `class`/`style` `fill:<name|hex>`
    /// (maps to ANSI foreground — readable on any terminal background).
    #[serde(default)]
    pub fill_color: Option<Color>,
    /// Border weight from `stroke-width`: `0` default, `1` heavy `┏━┓`
    /// (≥2px), `2` double `╔═╗` (≥3px).
    #[serde(default)]
    pub border_level: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ArrowDirection {
    #[default]
    Forward,
    Back,
    Both,
    None,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EdgeSpec {
    pub from: String,
    pub to: String,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub arrow: ArrowDirection,
    #[serde(default)]
    pub dashed: bool,
    #[serde(default)]
    pub thick: bool,
    /// Edge line + arrow color, via `linkStyle N stroke:<name|hex>` or JSON.
    #[serde(default)]
    pub color: Option<Color>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SubgraphSpec {
    pub id: String,
    #[serde(default)]
    pub title: Option<String>,
    /// Group-box border color, via `style <subgraph-id> stroke:<color>` or JSON.
    #[serde(default)]
    pub color: Option<Color>,
    /// Per-subgraph layout direction (`direction LR` inside the subgraph
    /// block, or JSON). Applied when the subgraph is edge-isolated from the
    /// rest of the diagram; otherwise the global direction wins (Mermaid
    /// parity).
    #[serde(default)]
    pub direction: Option<LayoutDirection>,
    /// Direct member node ids
    #[serde(default)]
    pub nodes: Vec<String>,
    /// Nested subgraphs
    #[serde(default)]
    pub subgraphs: Vec<SubgraphSpec>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct FlowchartSpec {
    #[serde(default)]
    pub direction: LayoutDirection,
    #[serde(default)]
    pub style: BoxStyle,
    #[serde(default)]
    pub title: Option<String>,
    pub nodes: Vec<NodeSpec>,
    pub edges: Vec<EdgeSpec>,
    #[serde(default)]
    pub subgraphs: Vec<SubgraphSpec>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ParticipantSpec {
    pub id: String,
    #[serde(default)]
    pub label: Option<String>,
    /// Participant box border + lifeline color (JSON `color` field).
    #[serde(default)]
    pub color: Option<Color>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum SeqMessageType {
    #[default]
    Sync, // ->
    Async,         // -->
    Reply,         // <--
    Bidirectional, // <->
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SeqMessageSpec {
    pub from: String,
    pub to: String,
    pub label: String,
    #[serde(default)]
    pub message_type: SeqMessageType,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SeqNoteSpec {
    pub over: Vec<String>,
    pub text: String,
    #[serde(default)]
    pub at_step: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SeqFrameSpec {
    /// Frame keyword: `alt`, `opt`, `loop`, `par`, `critical`, or `break`
    pub label: String,
    /// Branch condition texts in order (first = the frame's own condition)
    #[serde(default)]
    pub branches: Vec<String>,
    /// Message index at which each branch starts (parallel to `branches`)
    #[serde(default)]
    pub branch_steps: Vec<usize>,
    /// Index of the first message inside the frame
    pub start_step: usize,
    /// Index one past the last message inside the frame
    pub end_step: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct SequenceSpec {
    #[serde(default)]
    pub style: BoxStyle,
    #[serde(default)]
    pub title: Option<String>,
    pub participants: Vec<ParticipantSpec>,
    pub messages: Vec<SeqMessageSpec>,
    #[serde(default)]
    pub notes: Vec<SeqNoteSpec>,
    #[serde(default)]
    pub frames: Vec<SeqFrameSpec>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TreeNodeSpec {
    pub name: String,
    #[serde(default)]
    pub annotation: Option<String>,
    /// Branch-glyph color (JSON only — tree DSL has no color syntax).
    /// Children continuation glyphs inherit the nearest colored ancestor.
    #[serde(default)]
    pub color: Option<Color>,
    #[serde(default)]
    pub children: Vec<TreeNodeSpec>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TreeSpec {
    #[serde(default)]
    pub style: BoxStyle,
    pub root: TreeNodeSpec,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum TextAlign {
    #[default]
    Left,
    Center,
    Right,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TableSpec {
    #[serde(default)]
    pub style: BoxStyle,
    /// Grid/border color for the whole table (JSON only).
    #[serde(default)]
    pub color: Option<Color>,
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
    #[serde(default)]
    pub alignments: Vec<TextAlign>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StackLayerSpec {
    pub label: String,
    #[serde(default)]
    pub address_or_id: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    /// Layer box border color (JSON only).
    #[serde(default)]
    pub color: Option<Color>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StackSpec {
    #[serde(default)]
    pub style: BoxStyle,
    #[serde(default)]
    pub title: Option<String>,
    pub layers: Vec<StackLayerSpec>,
    #[serde(default)]
    pub bottom_address: Option<String>,
    #[serde(default)]
    pub grows_down: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ContainerLayout {
    #[default]
    Column,
    Row,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LeafComponent {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub properties: Vec<(String, String)>,
    /// Component border color (JSON `color` field).
    #[serde(default)]
    pub color: Option<Color>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ContainerItem {
    Leaf(LeafComponent),
    SubContainer(Box<ContainerSpec>),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ContainerSpec {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub layout: ContainerLayout,
    #[serde(default)]
    pub color: Option<Color>,
    #[serde(default)]
    pub items: Vec<ContainerItem>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ArchitectureSpec {
    #[serde(default)]
    pub style: BoxStyle,
    #[serde(default)]
    pub title: Option<String>,
    pub containers: Vec<ContainerSpec>,
    #[serde(default)]
    pub connections: Vec<EdgeSpec>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum DiagramSpec {
    Flowchart(FlowchartSpec),
    Sequence(SequenceSpec),
    Architecture(ArchitectureSpec),
    Tree(TreeSpec),
    Table(TableSpec),
    Stack(StackSpec),
}
