use crate::color::Color;
use crate::theme::BoxStyle;
use serde::{Deserialize, Serialize};

/// Coerces unquoted JSON numbers in data-structure specs to strings, so
/// `"values":[8,3,10]` and `"value":8` parse like their quoted forms.
/// Decimals normalize to their shortest round-trip form (`1.50` → `1.5`).
mod string_or_number {
    use serde::{Deserialize, Deserializer};
    use serde_json::Value;

    fn coerce(v: Value) -> Result<String, String> {
        match v {
            Value::String(s) => Ok(s),
            Value::Number(n) => Ok(n.to_string()),
            other => Err(format!("expected a string or number, got {other}")),
        }
    }

    pub fn string<'de, D: Deserializer<'de>>(d: D) -> Result<String, D::Error> {
        coerce(Value::deserialize(d)?).map_err(serde::de::Error::custom)
    }

    pub fn vec<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<String>, D::Error> {
        Vec::<Value>::deserialize(d)?
            .into_iter()
            .map(|v| coerce(v).map_err(serde::de::Error::custom))
            .collect()
    }
}

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

/// Data-structure diagram kind: `tree` (binary tree, `value`/`left`/`right`
/// nodes or `values` insertion order), `btree` (multi-key nodes with
/// `keys`/`children`), `linkedlist` (value chain in `nodes`), or `array`
/// (boxed cells from `values`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum DsKind {
    #[default]
    Tree,
    BTree,
    LinkedList,
    Array,
}

/// A node in a data-structure diagram. Binary trees use `value`/`left`/
/// `right`; B-trees use `keys`/`children`.
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct DsNode {
    /// Node label (binary tree kind). Numbers may be given unquoted in JSON.
    #[serde(default, deserialize_with = "string_or_number::string")]
    pub value: String,
    #[serde(default)]
    pub left: Option<Box<DsNode>>,
    #[serde(default)]
    pub right: Option<Box<DsNode>>,
    /// Keys of a B-tree node, rendered as `│ k1 │ k2 │` cells.
    #[serde(default, deserialize_with = "string_or_number::vec")]
    pub keys: Vec<String>,
    /// Child subtrees of a B-tree node (`keys.len()` separators imply
    /// `keys.len() + 1` children; fewer is rendered as-is).
    #[serde(default)]
    pub children: Vec<DsNode>,
    /// Node box border + connector color (JSON only). Descendants inherit
    /// the nearest colored ancestor unless they set their own; label text
    /// stays default.
    #[serde(default)]
    pub color: Option<Color>,
}

impl DsNode {
    /// Convenience constructor for a binary-tree leaf.
    #[must_use]
    pub fn leaf(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            ..Self::default()
        }
    }
}

/// Data-structure diagram: textbook-style trees with pointer links.
/// JSON-only input, e.g.
/// `{"type":"datastructure","kind":"tree","values":[8,3,10,1,6]}`.
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct DataStructureSpec {
    #[serde(default)]
    pub style: BoxStyle,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub kind: DsKind,
    /// Explicit binary-tree root (`kind: "tree"`).
    #[serde(default)]
    pub root: Option<DsNode>,
    /// Insertion order for building a BST (`kind: "tree"`); numeric strings
    /// compare numerically, others lexicographically. Ignored when `root`
    /// is present. Cell values for `kind: "array"`. Numbers may be given
    /// unquoted in JSON.
    #[serde(default, deserialize_with = "string_or_number::vec")]
    pub values: Vec<String>,
    /// B-tree root (`kind: "btree"`).
    #[serde(default)]
    pub btree_root: Option<DsNode>,
    /// Value chain for `kind: "linkedlist"`, rendered head → … → ∅.
    #[serde(default)]
    pub nodes: Vec<String>,
    /// Entry label drawn before the first linked-list node (default
    /// `"head"`).
    #[serde(default)]
    pub head_label: Option<String>,
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
    DataStructure(DataStructureSpec),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_json_unquoted_numbers_accepted() {
        let spec: DataStructureSpec =
            serde_json::from_str(r#"{"kind":"tree","values":[8,3,10,1.5]}"#).unwrap();
        assert_eq!(spec.values, vec!["8", "3", "10", "1.5"]);
    }

    #[test]
    fn test_json_unquoted_node_value_and_keys() {
        let spec: DataStructureSpec =
            serde_json::from_str(r#"{"kind":"btree","btree_root":{"value":8,"keys":[10,20]}}"#)
                .unwrap();
        let root = spec.btree_root.unwrap();
        assert_eq!(root.value, "8");
        assert_eq!(root.keys, vec!["10", "20"]);
    }

    #[test]
    fn test_json_rejects_non_scalar_cells() {
        for cell in ["true", "null", "[1]"] {
            let json = format!(r#"{{"kind":"array","values":[{cell}]}}"#);
            let err = serde_json::from_str::<DataStructureSpec>(&json)
                .expect_err("non-scalar cell must be rejected");
            assert!(
                err.to_string().contains("expected a string or number"),
                "cell {cell}: {err}"
            );
        }
    }

    #[test]
    fn test_unquoted_renders_same_as_quoted() {
        let unquoted: DataStructureSpec =
            serde_json::from_str(r#"{"kind":"tree","values":[8,3,10,1,6]}"#).unwrap();
        let quoted: DataStructureSpec =
            serde_json::from_str(r#"{"kind":"tree","values":["8","3","10","1","6"]}"#).unwrap();
        let a = crate::datastructure::DataStructureRenderer::new(
            &unquoted,
            crate::theme::Theme::ascii(),
        )
        .render(false)
        .unwrap();
        let b =
            crate::datastructure::DataStructureRenderer::new(&quoted, crate::theme::Theme::ascii())
                .render(false)
                .unwrap();
        assert_eq!(a, b);
    }
}
