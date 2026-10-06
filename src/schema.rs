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
    Rounded,    // (Text)
    Diamond,    // {Text} - Decision
    Database,   // [(Text)] - Cylinder
    Subprocess, // [[Text]]
    Stadium,    // ([Text]) - Pill
    Circle,     // ((Text))
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NodeSpec {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub shape: NodeShape,
    /// Dashed border, set via Mermaid `class`/`style` `stroke-dasharray`.
    /// Colors have no channel in a monochrome grid; dash patterns do.
    #[serde(default)]
    pub dashed_border: bool,
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
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ParticipantSpec {
    pub id: String,
    #[serde(default)]
    pub label: Option<String>,
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
