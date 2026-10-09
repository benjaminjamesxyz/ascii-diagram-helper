use crate::color::Color;
use crate::schema::{DataStructureSpec, DsKind, DsNode};
use crate::theme::{BoxStyle, Theme};
use std::borrow::Cow;
use std::fmt::Write;
use unicode_width::UnicodeWidthStr;

/// Paints `s` when `colored` and a color is set; otherwise returns it plain.
fn paint(colored: bool, color: Option<Color>, s: &str) -> String {
    match (colored, color) {
        (true, Some(c)) => c.paint(s),
        _ => s.to_string(),
    }
}

/// A single-line presentation, measured before any border color is applied.
struct DisplayLabel<'a> {
    text: Cow<'a, str>,
    width: usize,
}

impl<'a> DisplayLabel<'a> {
    fn new(raw: &'a str) -> Self {
        let special = raw.starts_with(char::is_whitespace)
            || raw.ends_with(char::is_whitespace)
            || raw
                .chars()
                .any(|c| c.is_control() || matches!(c, '"' | '\\'));
        let text = if special {
            let mut quoted = String::with_capacity(raw.len() + 2);
            quoted.push('"');
            for ch in raw.chars() {
                match ch {
                    '"' => quoted.push_str("\\\""),
                    '\\' => quoted.push_str("\\\\"),
                    '\n' => quoted.push_str("\\n"),
                    '\r' => quoted.push_str("\\r"),
                    '\t' => quoted.push_str("\\t"),
                    c if c.is_control() => {
                        write!(quoted, "\\u{:04x}", u32::from(c)).unwrap();
                    }
                    c => quoted.push(c),
                }
            }
            quoted.push('"');
            Cow::Owned(quoted)
        } else {
            Cow::Borrowed(raw)
        };
        let width = UnicodeWidthStr::width(text.as_ref());
        Self { text, width }
    }
}

/// A laid-out subtree: rendered lines (all equal display width), the display
/// width, and the column of the subtree root's box center.
struct Block {
    lines: Vec<String>,
    width: usize,
    center: usize,
}

/// Horizontal gap between sibling subtree blocks (leaves room for branches).
const SIBLING_GAP: usize = 4;

/// Maximum subtree depth `render_node` lays out before returning a clear
/// error. serde_json's 128-deep limit bounds explicit `root` trees; only
/// `values`-built BSTs can exceed this (sorted input builds a depth-n
/// chain). Depth-2000 verified working; crash observed between 2000-3000
/// pre-fix.
const MAX_RENDER_DEPTH: usize = 2048;

/// Maximum node count for `kind: "heap"`. A heap is a complete binary tree
/// (depth = log2 n), so the depth-based MAX_RENDER_DEPTH guard is
/// unreachable; heaps are bounded by value count instead.
const MAX_HEAP_NODES: usize = 2048;

/// Renders textbook-style data-structure diagrams: binary trees
/// (`kind: "tree"`), B-trees (`kind: "btree"`), singly and doubly linked
/// lists (`kind: "linkedlist"` / `"doublylinkedlist"`), arrays (`"array"`),
/// queues (`"queue"`), heaps (`"heap"`), and graphs (`"graph"`).
pub struct DataStructureRenderer<'a> {
    spec: &'a DataStructureSpec,
    theme: Theme,
}

impl<'a> DataStructureRenderer<'a> {
    #[must_use]
    pub fn new(spec: &'a DataStructureSpec, theme: Theme) -> Self {
        Self { spec, theme }
    }

    /// Renders the diagram.
    ///
    /// # Errors
    ///
    /// Returns `Err` when the spec carries nothing renderable: `tree` needs
    /// `root` or `values`, `btree` needs `btree_root`, either linked-list kind
    /// needs `nodes`, `array` needs `values`, `queue` needs `nodes`, `heap` needs
    /// `values`, and `graph` needs `nodes` (with every edge endpoint among
    /// them).
    pub fn render(&self, colored: bool) -> Result<String, String> {
        let (mut lines, width) = match self.spec.kind {
            DsKind::LinkedList => self.render_linkedlist()?,
            DsKind::DoublyLinkedList => self.render_doublylinkedlist()?,
            DsKind::Array => self.render_array()?,
            DsKind::Queue => self.render_queue()?,
            DsKind::Graph => self.render_graph()?,
            DsKind::Tree => {
                let root = self
                    .spec
                    .root
                    .clone()
                    .or_else(|| build_bst(&self.spec.values))
                    .ok_or_else(|| {
                        "tree diagram needs a `root` node or a `values` insertion order".to_string()
                    })?;
                let block = self.render_node(&root, None, colored, 0)?;
                (block.lines, block.width)
            }
            DsKind::BTree => {
                let root = self
                    .spec
                    .btree_root
                    .clone()
                    .ok_or_else(|| "btree diagram needs a `btree_root` node".to_string())?;
                let block = self.render_node(&root, None, colored, 0)?;
                (block.lines, block.width)
            }
            DsKind::Heap => {
                // A heap is a complete binary tree: children of the value at
                // index `i` sit at `2i + 1` and `2i + 2`. Building and
                // rendering go through the same tree layout as `tree`.
                // Depth grows logarithmically, so MAX_RENDER_DEPTH can never
                // fire here — heaps are bounded by NODE COUNT instead
                // (a 3000-value heap renders a 1.4 MB, 16k-column diagram).
                if self.spec.values.len() > MAX_HEAP_NODES {
                    return Err(format!(
                        "heap diagram exceeds the maximum node count ({MAX_HEAP_NODES}); a complete binary tree renders 2^depth wide rows - use fewer values"
                    ));
                }
                let root = build_heap(&self.spec.values)
                    .ok_or_else(|| "heap diagram needs a `values` list".to_string())?;
                let block = self.render_node(&root, None, colored, 0)?;
                (block.lines, block.width)
            }
        };

        if let Some(title) = &self.spec.title {
            let tw = UnicodeWidthStr::width(title.as_str());
            let indent = width.saturating_sub(tw) / 2;
            lines.insert(0, format!("{}{}", " ".repeat(indent), title));
            lines.insert(1, String::new());
        }

        Ok(lines
            .iter()
            .map(|l| l.trim_end().to_string())
            .collect::<Vec<_>>()
            .join("\n"))
    }

    /// Lays out a node box centered over its laid-out children, connected by
    /// a descender row, a branch bar, and child descenders. The node's own
    /// color overrides the inherited one for its box border and for the
    /// connector glyphs its subtree owns.
    ///
    /// # Errors
    ///
    /// Returns `Err` when the subtree is deeper than `MAX_RENDER_DEPTH`
    /// (a sorted `values` insertion order builds a tree as deep as the
    /// value count).
    fn render_node(
        &self,
        node: &DsNode,
        inherited: Option<Color>,
        colored: bool,
        depth: usize,
    ) -> Result<Block, String> {
        if depth > MAX_RENDER_DEPTH {
            return Err(format!(
                "data-structure diagram exceeds the maximum render depth \
                 ({MAX_RENDER_DEPTH}); a sorted `values` insertion order \
                 builds a tree as deep as the value count — use an explicit \
                 `root` or fewer values"
            ));
        }
        let effective = node.color.or(inherited);
        let (box_lines, box_w) = self.node_box(node, effective, colored);

        let child_nodes: Vec<&DsNode> = match self.spec.kind {
            DsKind::Tree | DsKind::Heap => [node.left.as_deref(), node.right.as_deref()]
                .iter()
                .flatten()
                .copied()
                .collect(),
            DsKind::BTree => node.children.iter().collect(),
            // Linear kinds render their own single-row layouts.
            DsKind::LinkedList
            | DsKind::DoublyLinkedList
            | DsKind::Array
            | DsKind::Queue
            | DsKind::Graph => Vec::new(),
        };
        // Recurse in a slim stack frame: debug-build frames for the full
        // layout body are far too wide to stack 2048 deep, so the recursion
        // lives in `render_children` and assembly runs in `assemble_block`.
        let child_blocks = self.render_children(&child_nodes, effective, colored, depth)?;
        let unary_left = (matches!(self.spec.kind, DsKind::Tree | DsKind::Heap)
            && child_blocks.len() == 1)
            .then_some(node.left.is_some());
        Ok(self.assemble_block(
            effective,
            colored,
            box_lines,
            box_w,
            child_blocks,
            unary_left,
        ))
    }

    /// Recurses into `children` one level deeper. Kept separate from
    /// `render_node` so per-level stack use stays minimal for deep chains.
    fn render_children(
        &self,
        children: &[&DsNode],
        effective: Option<Color>,
        colored: bool,
        depth: usize,
    ) -> Result<Vec<Block>, String> {
        children
            .iter()
            .map(|c| self.render_node(c, effective, colored, depth + 1))
            .collect()
    }

    /// Assembles one node's `Block` from its box lines and its laid-out
    /// child blocks: the box is centered over the children, connected by a
    /// descender row, a branch bar, and child descenders. The node's own
    /// `effective` color paints its box border and the connector glyphs its
    /// subtree owns.
    fn assemble_block(
        &self,
        effective: Option<Color>,
        colored: bool,
        box_lines: Vec<String>,
        box_w: usize,
        child_blocks: Vec<Block>,
        unary_left: Option<bool>,
    ) -> Block {
        if child_blocks.is_empty() {
            return Block {
                width: box_w,
                center: box_w / 2,
                lines: box_lines,
            };
        }

        let children_total: usize = child_blocks.iter().map(|b| b.width).sum::<usize>()
            + SIBLING_GAP * (child_blocks.len() - 1);
        let binary = matches!(self.spec.kind, DsKind::Tree | DsKind::Heap);
        let (box_x, child_start, sub_w) = if binary {
            // Center between actual child roots, not subtree bounds. A unary
            // slot offsets only two columns; absent subtrees never expand.
            let first = child_blocks[0].center;
            let desired_center = match unary_left {
                Some(true) => first + 2,
                Some(false) => first - 2,
                None => (first + child_blocks[0].width + SIBLING_GAP + child_blocks[1].center) / 2,
            };
            let child_start = (box_w / 2).saturating_sub(desired_center);
            let parent_center = desired_center + child_start;
            let box_x = parent_center - box_w / 2;
            let sub_w = (child_start + children_total).max(box_x + box_w);
            (box_x, child_start, sub_w)
        } else {
            let sub_w = box_w.max(children_total);
            ((sub_w - box_w) / 2, (sub_w - children_total) / 2, sub_w)
        };
        let parent_center = box_x + box_w / 2;

        let mut centers = Vec::with_capacity(child_blocks.len());
        let mut offset = child_start;
        for child in &child_blocks {
            centers.push(offset + child.center);
            offset += child.width + SIBLING_GAP;
        }

        // Node box, centered over the children and padded out to the
        // subtree width so every row stays exactly `sub_w` columns wide and
        // the box center lands on `parent_center`.
        let mut lines: Vec<String> = box_lines
            .iter()
            .map(|l| {
                format!(
                    "{}{l}{}",
                    " ".repeat(box_x),
                    " ".repeat(sub_w - box_x - box_w)
                )
            })
            .collect();

        // Binary ranks always use three connector rows; B-trees retain the
        // compact connector for an aligned single child.
        if !binary && centers.len() == 1 && centers[0] == parent_center {
            lines.push(painted_row(
                colored,
                effective,
                &spaced_row(parent_center, self.theme.vertical_line(), sub_w),
            ));
        } else {
            // Descender under the parent box
            lines.push(painted_row(
                colored,
                effective,
                &spaced_row(parent_center, self.theme.vertical_line(), sub_w),
            ));

            let mut bar = vec![' '; sub_w];
            let min = centers
                .iter()
                .copied()
                .min()
                .unwrap_or(parent_center)
                .min(parent_center);
            let max = centers
                .iter()
                .copied()
                .max()
                .unwrap_or(parent_center)
                .max(parent_center);
            bar[min..=max].fill(self.theme.horizontal_line());
            bar[parent_center] = if parent_center == min {
                self.theme.bottom_left_corner()
            } else if parent_center == max {
                self.theme.bottom_right_corner()
            } else {
                self.theme.tee_up()
            };
            for &c in &centers {
                bar[c] = if c == min {
                    self.theme.top_left_corner()
                } else if c == max {
                    self.theme.top_right_corner()
                } else if c == parent_center {
                    // Child aligned with parent descender: parent vertical,
                    // horizontal bar, and child vertical all meet -> 4-way cross.
                    self.theme.cross()
                } else {
                    self.theme.tee_down()
                };
            }
            let bar: String = bar.into_iter().collect();
            lines.push(painted_row(colored, effective, &bar));

            // Descenders into each child
            let mut drops = vec![' '; sub_w];
            for &c in &centers {
                drops[c] = self.theme.vertical_line();
            }
            let drops: String = drops.into_iter().collect();
            lines.push(painted_row(colored, effective, &drops));
        }

        // Paste child blocks side by side beneath the connectors
        let child_h = child_blocks
            .iter()
            .map(|b| b.lines.len())
            .max()
            .unwrap_or(0);
        for y in 0..child_h {
            let mut row = " ".repeat(child_start);
            for (i, child) in child_blocks.iter().enumerate() {
                if i > 0 {
                    row.push_str(&" ".repeat(SIBLING_GAP));
                }
                match child.lines.get(y) {
                    Some(l) => row.push_str(l),
                    None => row.push_str(&" ".repeat(child.width)),
                }
            }
            row.push_str(&" ".repeat(sub_w - child_start - children_total));
            lines.push(row);
        }

        Block {
            width: sub_w,
            center: parent_center,
            lines,
        }
    }

    /// Renders `kind: "linkedlist"`: a single left→right row of two-cell
    /// node boxes (`value │ pointer`), chained by theme arrows from the
    /// head label; the last pointer cell is a `∅` terminator (ASCII
    /// fallback `NULL`).
    fn render_linkedlist(&self) -> Result<(Vec<String>, usize), String> {
        if self.spec.nodes.is_empty() {
            return Err("linkedlist diagram needs a `nodes` value chain".to_string());
        }
        let head = DisplayLabel::new(self.spec.head_label.as_deref().unwrap_or("head"));
        let is_ascii = self.theme.box_style == BoxStyle::Ascii;
        let (pointer, null) = if is_ascii {
            ("X", "NULL")
        } else {
            ("●", "∅")
        };
        let pointer = DisplayLabel::new(pointer);
        let null = DisplayLabel::new(null);
        let values: Vec<_> = self
            .spec
            .nodes
            .iter()
            .map(|v| DisplayLabel::new(v))
            .collect();

        // Uniform value-cell width keeps the chain boxes visually level.
        let value_w = values.iter().map(|v| v.width).max().unwrap_or(1).max(1);
        let boxes: Vec<(Vec<String>, usize)> = values
            .iter()
            .enumerate()
            .map(|(i, v)| {
                let ptr = if i + 1 == self.spec.nodes.len() {
                    &null
                } else {
                    &pointer
                };
                self.cells_box_widths([(v, value_w), (ptr, ptr.width.max(1))], None, false)
            })
            .collect();

        let arrow = format!(
            "{}{}",
            self.theme.horizontal_line(),
            self.theme.arrow_right()
        );
        let conn = format!(" {arrow} ");
        let join_w = UnicodeWidthStr::width(conn.as_str());
        let joiner = " ".repeat(join_w);
        let pad = " ".repeat(head.width + join_w);

        let mut top = pad.clone();
        let mut mid = format!("{}{conn}", head.text);
        let mut bot = pad;
        for (i, (lines, _)) in boxes.iter().enumerate() {
            if i > 0 {
                top.push_str(&joiner);
                mid.push_str(&conn);
                bot.push_str(&joiner);
            }
            top.push_str(&lines[0]);
            mid.push_str(&lines[1]);
            bot.push_str(&lines[2]);
        }
        let lines = vec![top, mid, bot];
        let width = lines
            .iter()
            .map(|l| UnicodeWidthStr::width(l.as_str()))
            .max()
            .unwrap_or(0);
        Ok((lines, width))
    }

    /// Ordered prev/value/next cells. Adjacent boxes share both directions;
    /// only first.prev and last.next are null, including a singleton.
    fn render_doublylinkedlist(&self) -> Result<(Vec<String>, usize), String> {
        if self.spec.nodes.is_empty() {
            return Err("doublylinkedlist diagram needs a `nodes` value chain".to_string());
        }
        let head = DisplayLabel::new(self.spec.head_label.as_deref().unwrap_or("head"));
        let tail = DisplayLabel::new(self.spec.tail_label.as_deref().unwrap_or("tail"));
        let ascii = self.theme.box_style == BoxStyle::Ascii;
        let pointer = DisplayLabel::new(if ascii { "X" } else { "●" });
        let null = DisplayLabel::new(if ascii { "NULL" } else { "∅" });
        let values: Vec<_> = self
            .spec
            .nodes
            .iter()
            .map(|v| DisplayLabel::new(v))
            .collect();
        let value_w = values.iter().map(|v| v.width).max().unwrap_or(1).max(1);
        let pointer_w = pointer.width.max(null.width);
        let widths = [pointer_w, value_w, pointer_w];
        let forward = format!(
            " {}{} ",
            self.theme.horizontal_line(),
            self.theme.arrow_right()
        );
        let reverse = format!(
            " {}{} ",
            self.theme.arrow_left(),
            self.theme.horizontal_line()
        );
        let both = format!(
            " {}{}{} ",
            self.theme.arrow_left(),
            self.theme.horizontal_line(),
            self.theme.arrow_right()
        );
        let gap = " ".repeat(UnicodeWidthStr::width(both.as_str()));
        let pad = " ".repeat(head.width + UnicodeWidthStr::width(forward.as_str()));
        let mut lines = vec![pad.clone(), format!("{}{forward}", head.text), pad];
        for (i, value) in values.iter().enumerate() {
            if i > 0 {
                lines[0].push_str(&gap);
                lines[1].push_str(&both);
                lines[2].push_str(&gap);
            }
            let prev = if i == 0 { &null } else { &pointer };
            let next = if i + 1 == values.len() {
                &null
            } else {
                &pointer
            };
            let (cell_lines, _) =
                self.cells_box_widths([prev, value, next].into_iter().zip(widths), None, false);
            for (line, cell) in lines.iter_mut().zip(cell_lines) {
                line.push_str(&cell);
            }
        }
        lines[1].push_str(&reverse);
        lines[1].push_str(&tail.text);
        let width = UnicodeWidthStr::width(lines[1].as_str());
        Ok((lines, width))
    }

    /// Renders `kind: "array"`: one boxed row of cells with separators and
    /// a centered index ruler row above.
    fn render_array(&self) -> Result<(Vec<String>, usize), String> {
        let values: Vec<_> = self
            .spec
            .values
            .iter()
            .map(|v| DisplayLabel::new(v))
            .collect();
        if values.is_empty() {
            return Err("array diagram needs a `values` list".to_string());
        }
        // Per-cell width: value, index, or one column — whichever is
        // widest — so each ruler index centers over its cell.
        let widths: Vec<usize> = values
            .iter()
            .enumerate()
            .map(|(i, v)| {
                v.width
                    .max(UnicodeWidthStr::width(i.to_string().as_str()))
                    .max(1)
            })
            .collect();
        let (box_lines, _) =
            self.cells_box_widths(values.iter().zip(widths.iter().copied()), None, false);

        // Ruler indices centered in each cell block (` w `), with a blank
        // over every `│` separator column.
        let mut ruler = String::from(" ");
        for (i, &w) in widths.iter().enumerate() {
            if i > 0 {
                ruler.push(' ');
            }
            let idx = i.to_string();
            let pad = w.saturating_sub(UnicodeWidthStr::width(idx.as_str()));
            let left = pad / 2;
            let right = pad - left;
            ruler.push_str(&" ".repeat(1 + left));
            ruler.push_str(&idx);
            ruler.push_str(&" ".repeat(1 + right));
        }

        let mut lines = vec![ruler];
        lines.extend(box_lines);
        let width = lines
            .iter()
            .map(|l| UnicodeWidthStr::width(l.as_str()))
            .max()
            .unwrap_or(0);
        Ok((lines, width))
    }

    /// Renders `kind: "queue"`: a single left→right row of single-cell node
    /// boxes between the `front` and `rear` labels (`front_label`/
    /// `rear_label`, defaults `"front"`/`"rear"`). The front label points
    /// into the first box and the last box points at the rear label.
    /// Custom labels change text only; arrows remain rightward.
    fn render_queue(&self) -> Result<(Vec<String>, usize), String> {
        if self.spec.nodes.is_empty() {
            return Err("queue diagram needs a `nodes` list".to_string());
        }
        let front = DisplayLabel::new(self.spec.front_label.as_deref().unwrap_or("front"));
        let rear = DisplayLabel::new(self.spec.rear_label.as_deref().unwrap_or("rear"));
        let values: Vec<_> = self
            .spec
            .nodes
            .iter()
            .map(|v| DisplayLabel::new(v))
            .collect();

        // Uniform cell width keeps the boxes visually level.
        let cell_w = values.iter().map(|v| v.width).max().unwrap_or(1).max(1);
        let boxes: Vec<(Vec<String>, usize)> = values
            .iter()
            .map(|v| self.cells_box_widths([(v, cell_w)], None, false))
            .collect();

        let arrow = format!(
            "{}{}",
            self.theme.horizontal_line(),
            self.theme.arrow_right()
        );
        let conn = format!(" {arrow} ");
        let join_w = UnicodeWidthStr::width(conn.as_str());
        let joiner = " ".repeat(join_w);
        let pad = " ".repeat(front.width + join_w);

        let mut top = pad.clone();
        let mut mid = format!("{}{conn}", front.text);
        let mut bot = pad;
        for (i, (lines, _)) in boxes.iter().enumerate() {
            if i > 0 {
                top.push_str(&joiner);
                mid.push_str(&conn);
                bot.push_str(&joiner);
            }
            top.push_str(&lines[0]);
            mid.push_str(&lines[1]);
            bot.push_str(&lines[2]);
        }
        mid.push_str(&format!("{conn}{}", rear.text));
        let lines = vec![top, mid, bot];
        let width = lines
            .iter()
            .map(|l| UnicodeWidthStr::width(l.as_str()))
            .max()
            .unwrap_or(0);
        Ok((lines, width))
    }

    /// Renders `kind: "graph"` as a textbook adjacency list: one bucket row
    /// per source node (in `nodes` order) — the source's single-cell box,
    /// then a theme arrow into a chain of single-cell neighbor boxes, one
    /// per outgoing edge, duplicated in every bucket that references them.
    /// A self-loop renders as the source's own box in its own chain
    /// (`│ a │ ─► │ a │`) — the adjacency-list form of a loop-back edge.
    ///
    /// # Errors
    ///
    /// Returns `Err` when `nodes` is empty or contains duplicate identifiers,
    /// an edge is not a pair, or an edge references an unknown node.
    fn render_graph(&self) -> Result<(Vec<String>, usize), String> {
        if self.spec.nodes.is_empty() {
            return Err("graph diagram needs a `nodes` list".to_string());
        }
        let mut neighbors: Vec<Vec<usize>> = vec![Vec::new(); self.spec.nodes.len()];
        let mut index = std::collections::HashMap::with_capacity(self.spec.nodes.len());
        for (i, node) in self.spec.nodes.iter().enumerate() {
            if index.insert(node.as_str(), i).is_some() {
                return Err(format!("graph has duplicate node identifier: {node:?}"));
            }
        }
        for edge in &self.spec.edges {
            let [from, to] = edge.as_slice() else {
                return Err("graph edges must be [from, to] pairs".to_string());
            };
            let unknown: Vec<&str> = [from.as_str(), to.as_str()]
                .into_iter()
                .filter(|end| !index.contains_key(end))
                .collect();
            if !unknown.is_empty() {
                let list = unknown
                    .iter()
                    .map(|n| format!("\"{n}\""))
                    .collect::<Vec<_>>()
                    .join(", ");
                return Err(format!("graph edge references unknown node(s): {list}"));
            }
            let from_idx = index[from.as_str()];
            neighbors[from_idx].push(index[to.as_str()]);
        }

        // One uniform cell width across sources and neighbors keeps every
        // bucket box the same size, so rows align vertically.
        let labels: Vec<_> = self
            .spec
            .nodes
            .iter()
            .map(|n| DisplayLabel::new(n))
            .collect();
        let cell_w = labels
            .iter()
            .map(|label| label.width)
            .max()
            .unwrap_or(1)
            .max(1);
        let boxes: Vec<_> = labels
            .iter()
            .map(|label| self.cells_box_widths([(label, cell_w)], None, false).0)
            .collect();

        let arrow = format!(
            "{}{}",
            self.theme.horizontal_line(),
            self.theme.arrow_right()
        );
        let conn = format!(" {arrow} ");
        let join_w = UnicodeWidthStr::width(conn.as_str());
        let joiner = " ".repeat(join_w);

        let mut lines: Vec<String> = Vec::new();
        for (i, bucket) in neighbors.iter().enumerate() {
            if i > 0 {
                lines.push(String::new());
            }
            let source = &boxes[i];
            let mut top = source[0].clone();
            let mut mid = source[1].clone();
            let mut bot = source[2].clone();
            for &target in bucket {
                let cell_lines = &boxes[target];
                top.push_str(&joiner);
                top.push_str(&cell_lines[0]);
                mid.push_str(&conn);
                mid.push_str(&cell_lines[1]);
                bot.push_str(&joiner);
                bot.push_str(&cell_lines[2]);
            }
            lines.extend([top, mid, bot]);
        }
        let width = lines
            .iter()
            .map(|l| UnicodeWidthStr::width(l.as_str()))
            .max()
            .unwrap_or(0);
        Ok((lines, width))
    }

    /// Builds the 3-line node box: `┌────┐ / │ 10 │ 20 │ / └────┘`. Binary
    /// nodes render their single `value`; B-tree nodes render `keys` cells
    /// separated by the theme's vertical glyph. Border glyphs (corners,
    /// bars, cell separators) take `color`; label text stays default.
    fn node_box(&self, node: &DsNode, color: Option<Color>, colored: bool) -> (Vec<String>, usize) {
        // A keyless B-tree node uses `value`, just like a binary node.
        if !matches!(self.spec.kind, DsKind::BTree) || node.keys.is_empty() {
            let label = DisplayLabel::new(&node.value);
            return self.cells_box_widths([(&label, label.width.max(1))], color, colored);
        }
        let labels: Vec<_> = node.keys.iter().map(|key| DisplayLabel::new(key)).collect();
        let cell_w = labels
            .iter()
            .map(|label| label.width)
            .max()
            .unwrap_or(1)
            .max(1);
        self.cells_box_widths(labels.iter().map(|label| (label, cell_w)), color, colored)
    }

    /// Builds the 3-line box from displayed cells and their padded widths,
    /// separated by the theme's vertical glyph:
    /// `┌──┬──┐ / │ c1 │ c2 │ / └──┴──┘`. Border glyphs (corners,
    /// bars, separators) are painted `color` when `colored`; cell text is
    /// never painted, so labels stay terminal-default.
    fn cells_box_widths<'label, 'text: 'label>(
        &self,
        cells: impl IntoIterator<Item = (&'label DisplayLabel<'text>, usize)>,
        color: Option<Color>,
        colored: bool,
    ) -> (Vec<String>, usize) {
        let sep = paint(colored, color, &self.theme.vertical_line().to_string());
        let mut inner = String::new();
        let mut box_w = 1;
        for (i, (cell, width)) in cells.into_iter().enumerate() {
            if i > 0 {
                inner.push_str(&sep);
            }
            inner.push(' ');
            inner.push_str(&cell.text);
            inner.extend(std::iter::repeat_n(
                ' ',
                width.saturating_sub(cell.width) + 1,
            ));
            box_w += width + 3;
        }

        let (tl, tr, bl, br, h) = (
            self.theme.top_left_corner(),
            self.theme.top_right_corner(),
            self.theme.bottom_left_corner(),
            self.theme.bottom_right_corner(),
            self.theme.horizontal_line(),
        );
        let bar: String = std::iter::repeat_n(h, box_w - 2).collect();
        (
            vec![
                format!(
                    "{}{}{}",
                    paint(colored, color, &tl.to_string()),
                    paint(colored, color, &bar),
                    paint(colored, color, &tr.to_string())
                ),
                format!("{sep}{inner}{sep}"),
                format!(
                    "{}{}{}",
                    paint(colored, color, &bl.to_string()),
                    paint(colored, color, &bar),
                    paint(colored, color, &br.to_string())
                ),
            ],
            box_w,
        )
    }
}

/// One glyph at `x` in a blank row of `width` columns.
fn spaced_row(x: usize, ch: char, width: usize) -> String {
    let mut row = vec![' '; width];
    row[x] = ch;
    row.into_iter().collect()
}

/// Paints every glyph run in a connector row with `color` (when `colored`).
/// Contiguous non-space glyphs belong to the node that drew the row, so
/// wrapping each run keeps the SGR reset between glyphs and adjacent
/// children's own colors intact; zero-width escapes never shift layout.
fn painted_row(colored: bool, color: Option<Color>, row: &str) -> String {
    if !colored {
        return row.to_string();
    }
    match color {
        None => row.to_string(),
        Some(c) => {
            let mut out = String::with_capacity(row.len());
            let mut run = String::new();
            for ch in row.chars() {
                if ch == ' ' {
                    if !run.is_empty() {
                        out.push_str(&c.paint(&run));
                        run.clear();
                    }
                    out.push(ch);
                } else {
                    run.push(ch);
                }
            }
            if !run.is_empty() {
                out.push_str(&c.paint(&run));
            }
            out
        }
    }
}

/// Builds a binary search tree from an insertion order. Values that parse as
/// numbers compare numerically; anything else compares lexicographically.
fn build_bst(values: &[String]) -> Option<DsNode> {
    let mut root: Option<Box<DsNode>> = None;
    for value in values {
        insert_bst(&mut root, value.clone());
    }
    root.map(|b| *b)
}

fn insert_bst(node: &mut Option<Box<DsNode>>, value: String) {
    // Iterative descent: a sorted insertion order builds a depth-n chain,
    // which would overflow the stack if inserted recursively.
    let mut cur = node;
    loop {
        match cur {
            None => {
                *cur = Some(Box::new(DsNode::leaf(value)));
                return;
            }
            Some(n) => {
                let goes_left = less_than(&value, &n.value);
                cur = if goes_left { &mut n.left } else { &mut n.right };
            }
        }
    }
}

fn less_than(a: &str, b: &str) -> bool {
    if let (Some((a_negative, a_digits)), Some((b_negative, b_digits))) =
        (integer_parts(a), integer_parts(b))
    {
        let magnitude = a_digits
            .len()
            .cmp(&b_digits.len())
            .then_with(|| a_digits.cmp(b_digits));
        return match (a_negative, b_negative) {
            (true, false) => true,
            (false, true) => false,
            (true, true) => magnitude.is_gt(),
            (false, false) => magnitude.is_lt(),
        };
    }
    match (a.parse::<f64>(), b.parse::<f64>()) {
        (Ok(x), Ok(y)) => x < y,
        _ => a < b,
    }
}

/// Normalize only integral tokens; retain the original label in the tree.
fn integer_parts(value: &str) -> Option<(bool, &str)> {
    let digits = value.strip_prefix(['+', '-']).unwrap_or(value);
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let magnitude = digits.trim_start_matches('0');
    Some((value.starts_with('-') && !magnitude.is_empty(), magnitude))
}

/// Builds a complete binary tree from `values` by index: the children of
/// the value at `i` sit at `2i + 1` and `2i + 2`. Links are attached
/// iteratively from the last index down, so large inputs never recurse.
fn build_heap(values: &[String]) -> Option<DsNode> {
    let mut nodes: Vec<DsNode> = values.iter().map(|v| DsNode::leaf(v.clone())).collect();
    for i in (0..nodes.len()).rev() {
        let left =
            (2 * i + 1 < nodes.len()).then(|| Box::new(std::mem::take(&mut nodes[2 * i + 1])));
        let right =
            (2 * i + 2 < nodes.len()).then(|| Box::new(std::mem::take(&mut nodes[2 * i + 2])));
        nodes[i].left = left;
        nodes[i].right = right;
    }
    nodes.into_iter().next()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::DiagramSpec;
    use crate::theme::BoxStyle;

    fn spec(kind: DsKind, json_root: DsNode) -> DataStructureSpec {
        let (root, btree_root) = match kind {
            DsKind::Tree => (Some(json_root), None),
            _ => (None, Some(json_root)),
        };
        DataStructureSpec {
            style: crate::theme::BoxStyle::Sharp,
            kind,
            root,
            btree_root,
            ..DataStructureSpec::default()
        }
    }

    #[test]
    fn test_bst_from_values() {
        let spec = DataStructureSpec {
            style: BoxStyle::Sharp,
            kind: DsKind::Tree,
            values: ["8", "3", "10", "1"]
                .iter()
                .map(|s| (*s).to_string())
                .collect(),
            ..DataStructureSpec::default()
        };
        let out = DataStructureRenderer::new(&spec, Theme::ascii())
            .render(false)
            .unwrap();
        assert!(out.contains("| 8 |"));
        assert!(out.contains("| 1 |"));
        // 3 must sit left of 10 under root 8
        let idx3 = out.lines().position(|l| l.contains("| 3 |")).unwrap();
        let idx10 = out.lines().position(|l| l.contains("| 10 |")).unwrap();
        assert_eq!(idx3, idx10, "children share a row");
    }

    #[test]
    fn test_explicit_binary_tree_shape() {
        let root = DsNode {
            value: "8".into(),
            left: Some(Box::new(DsNode::leaf("3"))),
            right: Some(Box::new(DsNode::leaf("10"))),
            ..DsNode::default()
        };
        let out =
            DataStructureRenderer::new(&spec(DsKind::Tree, root), Theme::ascii()).render(false);
        let out = out.unwrap();
        assert!(out.contains('+'), "ASCII theme uses ASCII glyphs");
        assert!(!out.contains('│'), "ASCII theme has no Unicode glyphs");
        assert!(out.contains('-'), "ASCII theme uses ASCII lines");
    }

    #[test]
    fn test_btree_keys_and_children() {
        let root = DsNode {
            keys: vec!["10".into(), "20".into()],
            children: vec![
                DsNode {
                    keys: vec!["3".into(), "5".into()],
                    ..DsNode::default()
                },
                DsNode {
                    keys: vec!["12".into(), "15".into()],
                    ..DsNode::default()
                },
                DsNode {
                    keys: vec!["25".into(), "30".into()],
                    ..DsNode::default()
                },
            ],
            ..DsNode::default()
        };
        let out =
            DataStructureRenderer::new(&spec(DsKind::BTree, root), Theme::new(BoxStyle::Sharp))
                .render(false)
                .unwrap();
        assert!(out.contains("│ 10 │ 20 │"));
        assert!(out.contains("│ 3 │ 5 │"));
        assert!(out.contains("│ 25 │ 30 │"));
        assert!(out.contains('┴'), "parent descender meets branch bar");
        assert_eq!(out.matches('┬').count(), 1, "middle child gets a tee-down");
    }

    #[test]
    fn test_title_centered() {
        let out = DataStructureRenderer::new(
            &DataStructureSpec {
                style: BoxStyle::Sharp,
                title: Some("BST".into()),
                values: vec!["1".into()],
                ..DataStructureSpec::default()
            },
            Theme::new(BoxStyle::Sharp),
        )
        .render(false)
        .unwrap();
        let first = out.lines().next().unwrap();
        assert_eq!(first.trim(), "BST");
    }

    #[test]
    fn test_missing_roots_error() {
        let tree = DataStructureSpec {
            style: BoxStyle::Sharp,
            kind: DsKind::Tree,
            ..DataStructureSpec::default()
        };
        assert!(
            DataStructureRenderer::new(&tree, Theme::ascii())
                .render(false)
                .is_err()
        );
        let btree = DataStructureSpec {
            style: BoxStyle::Sharp,
            kind: DsKind::BTree,
            ..DataStructureSpec::default()
        };
        assert!(
            DataStructureRenderer::new(&btree, Theme::ascii())
                .render(false)
                .is_err()
        );
    }

    #[test]
    fn test_numeric_vs_lexicographic_ordering() {
        // 10 < 9 numerically; lexicographically "10" < "9" too, so use a case
        // that differs: 2 < 10 numerically but "2" > "10" as strings.
        let spec = DataStructureSpec {
            style: BoxStyle::Sharp,
            kind: DsKind::Tree,
            values: vec!["2".into(), "10".into()],
            ..DataStructureSpec::default()
        };
        let out = DataStructureRenderer::new(&spec, Theme::ascii())
            .render(false)
            .unwrap();
        let row_of = |needle: &str| {
            out.lines()
                .position(|l| l.contains(needle))
                .expect("node present")
        };
        assert!(
            row_of("| 10 |") > row_of("| 2 |"),
            "10 descends below root 2"
        );
    }

    fn linked_spec(nodes: &[&str]) -> DataStructureSpec {
        DataStructureSpec {
            style: BoxStyle::Sharp,
            kind: DsKind::LinkedList,
            nodes: nodes.iter().map(|s| (*s).to_string()).collect(),
            ..DataStructureSpec::default()
        }
    }

    fn array_spec(values: &[&str]) -> DataStructureSpec {
        DataStructureSpec {
            style: BoxStyle::Sharp,
            kind: DsKind::Array,
            values: values.iter().map(|s| (*s).to_string()).collect(),
            ..DataStructureSpec::default()
        }
    }

    #[test]
    fn test_linkedlist_chain() {
        let out = DataStructureRenderer::new(
            &linked_spec(&["10", "20", "30"]),
            Theme::new(BoxStyle::Sharp),
        )
        .render(false)
        .unwrap();
        assert!(out.contains("│ 10 │ ● │"), "two-cell node box: {out}");
        assert!(out.contains("∅"), "last pointer cell is the NULL glyph");
        assert!(out.contains("►"), "theme arrow between boxes");
        assert!(!out.contains("││"), "boxes separated by arrows: {out}");
        assert_eq!(out.matches("head").count(), 1, "default head label");
        assert_eq!(out.lines().count(), 3, "single-row layout");
        assert!(
            out.find("head").unwrap() < out.find("│ 10 │").unwrap()
                && out.find("│ 10 │").unwrap() < out.find("│ 30 │").unwrap()
                && out.find("│ 30 │").unwrap() < out.find("∅").unwrap(),
            "head → chain → NULL order"
        );
    }

    #[test]
    fn test_linkedlist_ascii_fallback() {
        let out = DataStructureRenderer::new(&linked_spec(&["10", "20"]), Theme::ascii())
            .render(false)
            .unwrap();
        assert!(out.contains("| 10 | X |"), "ascii pointer cell: {out}");
        assert!(out.contains("NULL"), "ascii NULL terminator");
        assert!(out.contains("->"), "ascii arrow");
        assert!(
            !out.contains('●') && !out.contains('∅') && !out.contains('►') && !out.contains('│'),
            "ascii theme has no Unicode glyphs"
        );
    }

    #[test]
    fn test_linkedlist_custom_head_label() {
        let spec = DataStructureSpec {
            head_label: Some("front".into()),
            ..linked_spec(&["1"])
        };
        let out = DataStructureRenderer::new(&spec, Theme::new(BoxStyle::Sharp))
            .render(false)
            .unwrap();
        assert!(out.contains("front ─► │ 1 │ ∅ │"), "{out}");
        assert!(!out.contains("head"));
    }

    #[test]
    fn test_linkedlist_needs_nodes() {
        assert!(
            DataStructureRenderer::new(&linked_spec(&[]), Theme::ascii())
                .render(false)
                .is_err()
        );
    }

    #[test]
    fn test_array_ruler_and_cells() {
        let out =
            DataStructureRenderer::new(&array_spec(&["a", "b", "c"]), Theme::new(BoxStyle::Sharp))
                .render(false)
                .unwrap();
        assert!(out.contains("│ a │ b │ c │"), "boxed cell row: {out}");
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines.len(), 4, "ruler row + 3 box lines");
        let (ruler, content) = (lines[0], lines[2]);
        assert!(ruler.contains('0') && ruler.contains('1') && ruler.contains('2'));
        // Compare char columns — byte offsets skew on multi-byte `│`.
        let col = |s: &str, c: char| s.chars().position(|ch| ch == c).unwrap();
        for (idx, val) in [('0', 'a'), ('1', 'b'), ('2', 'c')] {
            assert_eq!(
                col(ruler, idx),
                col(content, val),
                "index centered over its cell"
            );
        }
    }

    #[test]
    fn test_array_ascii_fallback() {
        let out = DataStructureRenderer::new(&array_spec(&["a", "b"]), Theme::ascii())
            .render(false)
            .unwrap();
        assert!(out.contains("| a | b |"), "ascii cells: {out}");
        assert!(out.contains('+') && out.contains('-'), "ascii box");
        assert!(out.contains('0') && out.contains('1'), "index ruler");
        assert!(
            !out.contains('│') && !out.contains('┌'),
            "no Unicode glyphs"
        );
    }

    #[test]
    fn test_array_needs_values() {
        assert!(
            DataStructureRenderer::new(&array_spec(&[]), Theme::ascii())
                .render(false)
                .is_err()
        );
    }

    fn queue_spec(nodes: &[&str]) -> DataStructureSpec {
        DataStructureSpec {
            style: BoxStyle::Sharp,
            kind: DsKind::Queue,
            nodes: nodes.iter().map(|s| (*s).to_string()).collect(),
            ..DataStructureSpec::default()
        }
    }

    fn heap_spec(values: &[&str]) -> DataStructureSpec {
        DataStructureSpec {
            style: BoxStyle::Sharp,
            kind: DsKind::Heap,
            values: values.iter().map(|s| (*s).to_string()).collect(),
            ..DataStructureSpec::default()
        }
    }

    fn graph_spec(nodes: &[&str], edges: &[&[&str; 2]]) -> DataStructureSpec {
        DataStructureSpec {
            style: BoxStyle::Sharp,
            kind: DsKind::Graph,
            nodes: nodes.iter().map(|s| (*s).to_string()).collect(),
            edges: edges
                .iter()
                .map(|pair| pair.iter().map(|s| (*s).to_string()).collect())
                .collect(),
            ..DataStructureSpec::default()
        }
    }

    #[test]
    fn test_queue_row_front_to_rear() {
        let out =
            DataStructureRenderer::new(&queue_spec(&["a", "b", "c"]), Theme::new(BoxStyle::Sharp))
                .render(false)
                .unwrap();
        assert!(
            out.contains("front ─► │ a │"),
            "arrow into first box: {out}"
        );
        assert!(
            out.contains("│ c │ ─► rear"),
            "arrow out of last box: {out}"
        );
        assert!(out.contains('►'), "theme arrow glyphs");
        assert!(!out.contains("││"), "boxes separated by arrows: {out}");
        assert_eq!(out.lines().count(), 3, "single-row layout");
        let order = ["front", "│ a │", "│ b │", "│ c │", "rear"]
            .iter()
            .map(|needle| out.find(needle).unwrap())
            .collect::<Vec<_>>();
        assert!(
            order.windows(2).all(|w| w[0] < w[1]),
            "front → chain → rear order"
        );
        assert_eq!(out.matches("front").count(), 1);
        assert_eq!(out.matches("rear").count(), 1);
    }

    #[test]
    fn test_queue_ascii_fallback() {
        let out = DataStructureRenderer::new(&queue_spec(&["a", "b"]), Theme::ascii())
            .render(false)
            .unwrap();
        assert!(out.contains("front -> | a | -> | b | -> rear"), "{out}");
        assert!(
            !out.contains('►') && !out.contains('│') && !out.contains('─'),
            "ascii theme has no Unicode glyphs"
        );
    }

    #[test]
    fn test_queue_custom_labels_keep_rightward_arrows() {
        let spec = DataStructureSpec {
            front_label: Some("push_back".into()),
            rear_label: Some("push_front".into()),
            ..queue_spec(&["1"])
        };
        let out = DataStructureRenderer::new(&spec, Theme::new(BoxStyle::Sharp))
            .render(false)
            .unwrap();
        assert!(out.contains("push_back ─► │ 1 │ ─► push_front"), "{out}");
        assert!(!out.contains("front ") && !out.contains(" rear"), "{out}");
    }

    #[test]
    fn test_queue_needs_nodes() {
        assert!(
            DataStructureRenderer::new(&queue_spec(&[]), Theme::ascii())
                .render(false)
                .is_err()
        );
    }

    #[test]
    fn test_heap_complete_tree_by_index() {
        // values [1,2,3,4,5,6,7] build a perfect tree: 1 over (2,3),
        // 2 over (4,5), 3 over (6,7).
        let out = DataStructureRenderer::new(
            &heap_spec(&["1", "2", "3", "4", "5", "6", "7"]),
            Theme::ascii(),
        )
        .render(false)
        .unwrap();
        let row_of = |needle: &str| {
            out.lines()
                .position(|l| l.contains(needle))
                .unwrap_or_else(|| panic!("{needle} present: {out}"))
        };
        let (r1, r2, r3) = (row_of("| 1 |"), row_of("| 2 |"), row_of("| 3 |"));
        assert!(r1 < r2 && r2 == r3, "2 and 3 share the child row: {out}");
        assert_eq!(row_of("| 4 |"), row_of("| 5 |"), "4 and 5 share a row");
        assert_eq!(row_of("| 6 |"), row_of("| 7 |"), "6 and 7 share a row");
        assert!(row_of("| 4 |") > r2, "grandchildren below their parent");
        let col = |s: &str, c: char| {
            out.lines()
                .nth(row_of(s))
                .unwrap()
                .chars()
                .position(|ch| ch == c)
                .unwrap()
        };
        assert!(col("| 2 |", '2') < col("| 1 |", '1') && col("| 1 |", '1') < col("| 3 |", '3'));
        assert!(col("| 4 |", '4') < col("| 2 |", '2') && col("| 5 |", '5') > col("| 2 |", '2'));
    }

    #[test]
    fn test_heap_unicode_and_ascii_glyphs() {
        let unicode =
            DataStructureRenderer::new(&heap_spec(&["1", "2"]), Theme::new(BoxStyle::Sharp))
                .render(false)
                .unwrap();
        assert!(unicode.contains('┌') && unicode.contains('│'), "{unicode}");
        assert!(unicode.contains("│ 1 │") && unicode.contains("│ 2 │"));
        let ascii = DataStructureRenderer::new(&heap_spec(&["1", "2"]), Theme::ascii())
            .render(false)
            .unwrap();
        assert!(ascii.contains('+') && ascii.contains('-') && ascii.contains('|'));
        assert!(!ascii.contains('│') && !ascii.contains('┌'), "no Unicode");
    }

    #[test]
    fn test_heap_single_value_and_odd_count() {
        let single = DataStructureRenderer::new(&heap_spec(&["42"]), Theme::ascii())
            .render(false)
            .unwrap();
        assert!(single.contains("| 42 |"), "{single}");
        assert_eq!(single.lines().count(), 3, "lone root has no connectors");
        // 5 values: last level partially filled — 4 and 5 are 2's children.
        let odd =
            DataStructureRenderer::new(&heap_spec(&["1", "2", "3", "4", "5"]), Theme::ascii())
                .render(false)
                .unwrap();
        let row_of = |needle: &str| {
            odd.lines()
                .position(|l| l.contains(needle))
                .unwrap_or_else(|| panic!("{needle} present: {odd}"))
        };
        assert_eq!(row_of("| 4 |"), row_of("| 5 |"), "both under 2");
        assert!(row_of("| 3 |") < row_of("| 4 |"), "3's leaf row is last");
    }

    #[test]
    fn test_heap_needs_values() {
        assert!(
            DataStructureRenderer::new(&heap_spec(&[]), Theme::ascii())
                .render(false)
                .is_err()
        );
    }

    #[test]
    fn test_graph_adjacency_buckets() {
        let out = DataStructureRenderer::new(
            &graph_spec(&["a", "b", "c"], &[&["a", "b"], &["a", "c"], &["b", "c"]]),
            Theme::new(BoxStyle::Sharp),
        )
        .render(false)
        .unwrap();
        let lines: Vec<&str> = out.lines().collect();
        // Bucket rows separated by blank lines: 3 buckets → 3*3 + 2 lines.
        assert_eq!(lines.len(), 11, "one 3-line row per source bucket: {out}");
        assert_eq!(lines[1], "│ a │ ─► │ b │ ─► │ c │", "a's bucket: {out}");
        assert_eq!(lines[5], "│ b │ ─► │ c │", "b's bucket: {out}");
        assert_eq!(lines[9], "│ c │", "edgeless source renders alone: {out}");
        assert_eq!(
            out.matches("│ c │").count(),
            3,
            "c in a's + b's buckets plus its own source box"
        );
        assert_eq!(out.matches("│ a │").count(), 1, "a sources only itself");
    }

    #[test]
    fn test_graph_ascii_fallback() {
        let out =
            DataStructureRenderer::new(&graph_spec(&["a", "b"], &[&["a", "b"]]), Theme::ascii())
                .render(false)
                .unwrap();
        assert!(out.contains("| a | -> | b |"), "{out}");
        assert!(
            !out.contains('►') && !out.contains('│') && !out.contains('─'),
            "ascii theme has no Unicode glyphs"
        );
    }

    #[test]
    fn test_graph_self_loop_is_own_bucket_cell() {
        let out = DataStructureRenderer::new(
            &graph_spec(&["a", "b"], &[&["a", "a"], &["a", "b"]]),
            Theme::new(BoxStyle::Sharp),
        )
        .render(false)
        .unwrap();
        assert!(
            out.contains("│ a │ ─► │ a │ ─► │ b │"),
            "self-loop renders as the source's own cell in its chain: {out}"
        );
    }

    #[test]
    fn test_graph_unknown_endpoint_named() {
        let one = DataStructureRenderer::new(&graph_spec(&["a"], &[&["a", "zz"]]), Theme::ascii())
            .render(false)
            .unwrap_err();
        assert!(one.contains('"'), "names the unknown node: {one}");
        assert!(one.contains("zz"), "{one}");
        let both = DataStructureRenderer::new(&graph_spec(&["a"], &[&["x", "y"]]), Theme::ascii())
            .render(false)
            .unwrap_err();
        assert!(both.contains("x") && both.contains("y"), "{both}");
    }

    #[test]
    fn test_graph_needs_nodes_and_valid_pairs() {
        assert!(
            DataStructureRenderer::new(&graph_spec(&[], &[]), Theme::ascii())
                .render(false)
                .is_err()
        );
        for bad in [vec!["a"], vec!["a", "b", "c"]] {
            let edges: Vec<Vec<String>> = vec![bad.iter().map(|s| (*s).to_string()).collect()];
            let spec = DataStructureSpec {
                style: BoxStyle::Sharp,
                kind: DsKind::Graph,
                nodes: vec!["a".into(), "b".into()],
                edges,
                ..DataStructureSpec::default()
            };
            let err = DataStructureRenderer::new(&spec, Theme::ascii())
                .render(false)
                .unwrap_err();
            assert!(err.contains("[from, to] pairs"), "{err}");
        }
    }

    #[test]
    fn test_queue_heap_graph_colored_matches_plain() {
        let specs = [
            queue_spec(&["a", "b"]),
            heap_spec(&["1", "2", "3"]),
            graph_spec(&["a", "b"], &[&["a", "b"]]),
        ];
        for spec in specs {
            let ds = DiagramSpec::DataStructure(spec);
            let plain = crate::render_diagram_colored(&ds, false).unwrap();
            let colored = crate::render_diagram_colored(&ds, true).unwrap();
            assert_eq!(
                plain, colored,
                "color-less kinds carry no color source — identical output"
            );
            assert!(!plain.contains('\x1b'), "plain render has zero ANSI");
        }
    }

    #[test]
    fn test_linkedlist_and_array_colored_matches_plain() {
        for spec in [linked_spec(&["10", "20"]), array_spec(&["a", "b"])] {
            let ds = DiagramSpec::DataStructure(spec);
            let plain = crate::render_diagram_colored(&ds, false).unwrap();
            let colored = crate::render_diagram_colored(&ds, true).unwrap();
            assert_eq!(
                plain, colored,
                "linear kinds carry no color source — identical output"
            );
            assert!(!plain.contains('\x1b'), "plain render has zero ANSI");
        }
    }

    /// Root 8 (red), left leaf 3 (inherits red), right leaf 10 (blue).
    fn color_tree() -> DsNode {
        DsNode {
            value: "8".into(),
            color: Some(Color::Red),
            left: Some(Box::new(DsNode::leaf("3"))),
            right: Some(Box::new(DsNode {
                value: "10".into(),
                color: Some(Color::Blue),
                ..DsNode::default()
            })),
            ..DsNode::default()
        }
    }

    #[test]
    fn test_plain_render_has_no_ansi() {
        let tree = spec(DsKind::Tree, color_tree());
        let out = DataStructureRenderer::new(&tree, Theme::new(BoxStyle::Sharp))
            .render(false)
            .unwrap();
        assert!(
            !out.contains('\u{1b}'),
            "plain render must stay byte-identical (zero ANSI)"
        );
    }

    #[test]
    fn test_colored_render_emits_sgr_and_keeps_layout() {
        let tree = spec(DsKind::Tree, color_tree());
        let renderer = DataStructureRenderer::new(&tree, Theme::new(BoxStyle::Sharp));
        let plain = renderer.render(false).unwrap();
        let colored = renderer.render(true).unwrap();
        assert_eq!(
            crate::color::Color::strip_ansi(&colored),
            plain,
            "color never shifts layout"
        );
        // Root box border painted red: corner + bar as separate SGR runs.
        assert!(
            colored.contains("\u{1b}[31m\u{250c}\u{1b}[39m\u{1b}[31m"),
            "root border red: {colored:?}"
        );
        // Root connector glyphs (owned by its subtree) painted red too.
        assert!(
            colored.contains("\u{1b}[31m\u{250c}\u{2500}"),
            "connector run red"
        );
        // Label text stays default: no SGR run starts inside a cell.
        assert!(!colored.contains("[31m 8 "), "label text unpainted");
    }

    #[test]
    fn test_color_inheritance_and_override() {
        let tree = spec(DsKind::Tree, color_tree());
        let renderer = DataStructureRenderer::new(&tree, Theme::new(BoxStyle::Sharp));
        let colored = renderer.render(true).unwrap();
        // Left leaf inherits red from the root: its box separators are red.
        assert!(
            colored.contains("\u{1b}[39m 3 \u{1b}[31m"),
            "inherited color on child border: {colored:?}"
        );
        // Right leaf overrides blue: blue separators around its own label.
        assert!(
            colored.contains("\u{1b}[34m\u{2502}\u{1b}[39m 10 "),
            "per-node override wins: {colored:?}"
        );
    }

    #[test]
    fn test_uncolored_spec_render_identical_when_colored() {
        let spec = DataStructureSpec {
            style: BoxStyle::Sharp,
            kind: DsKind::Tree,
            values: vec!["8".into(), "3".into(), "10".into()],
            ..DataStructureSpec::default()
        };
        let renderer = DataStructureRenderer::new(&spec, Theme::new(BoxStyle::Sharp));
        assert_eq!(
            renderer.render(true).unwrap(),
            renderer.render(false).unwrap(),
            "no colors set means colored render is byte-identical"
        );
    }

    #[test]
    fn test_btree_node_color() {
        let root = DsNode {
            keys: vec!["10".into(), "20".into()],
            color: Some(Color::Red),
            ..DsNode::default()
        };
        let btree = spec(DsKind::BTree, root);
        let colored = DataStructureRenderer::new(&btree, Theme::new(BoxStyle::Sharp))
            .render(true)
            .unwrap();
        // Cell separators painted; key text stays default.
        assert!(
            colored.contains("\u{1b}[31m\u{2502}\u{1b}[39m 10 "),
            "btree separators red, keys default: {colored:?}"
        );
        assert!(!colored.contains("[31m 20 "), "key text unpainted");
    }

    /// Runs `f` on a thread with a large stack: deep-tree renders recurse
    /// ~2000 levels, which fits the 8 MiB main thread the CLI runs on but
    /// not libtest's default 2 MiB worker threads.
    fn with_big_stack<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
        std::thread::Builder::new()
            .stack_size(256 * 1024 * 1024)
            .spawn(f)
            .unwrap()
            .join()
            .unwrap()
    }

    #[test]
    fn test_bst_sorted_3000_no_stack_overflow() {
        // Sorted input builds a depth-3000 chain. Iterative `insert_bst`
        // must build it without overflowing, and the render depth guard must
        // reject the layout with a clean error (never a stack overflow).
        with_big_stack(|| {
            let values: Vec<String> = (1..=3000).map(|v| v.to_string()).collect();
            let spec = DataStructureSpec {
                style: BoxStyle::Sharp,
                kind: DsKind::Tree,
                values,
                ..DataStructureSpec::default()
            };
            let err = DataStructureRenderer::new(&spec, Theme::ascii())
                .render(false)
                .unwrap_err();
            assert!(
                err.contains("maximum render depth"),
                "3000 sorted values must hit the depth guard cleanly, got: {err}"
            );
        });
    }

    #[test]
    fn test_bst_sorted_2000_still_supported() {
        with_big_stack(|| {
            let values: Vec<String> = (1..=2000).map(|v| v.to_string()).collect();
            let spec = DataStructureSpec {
                style: BoxStyle::Sharp,
                kind: DsKind::Tree,
                values,
                ..DataStructureSpec::default()
            };
            let out = DataStructureRenderer::new(&spec, Theme::ascii())
                .render(false)
                .unwrap();
            assert!(out.contains("| 2000 |"));
        });
    }

    #[test]
    fn test_render_depth_guard_explicit_chain() {
        // A 3000-deep explicit left chain (programmatic, so serde_json's own
        // 128-level limit does not apply) must return the depth-guard error.
        with_big_stack(|| {
            let mut node = DsNode::leaf("0");
            for i in 1..=3000 {
                node = DsNode {
                    value: i.to_string(),
                    left: Some(Box::new(node)),
                    ..DsNode::default()
                };
            }
            let out = DataStructureRenderer::new(&spec(DsKind::Tree, node), Theme::ascii())
                .render(false)
                .unwrap_err();
            assert!(
                out.contains("maximum render depth"),
                "explicit deep chain must hit the depth guard, got: {out}"
            );
        });
    }

    #[test]
    fn test_btree_root_box_indented_over_children() {
        // 3-child btree (keys 10,20): root box must be left-indented so its
        // center sits over the middle child, and the bar tee marks that spot.
        let root = DsNode {
            keys: vec!["10".into(), "20".into()],
            children: vec![
                DsNode {
                    keys: vec!["3".into(), "5".into()],
                    ..DsNode::default()
                },
                DsNode {
                    keys: vec!["12".into(), "15".into()],
                    ..DsNode::default()
                },
                DsNode {
                    keys: vec!["25".into(), "30".into()],
                    ..DsNode::default()
                },
            ],
            ..DsNode::default()
        };
        let out =
            DataStructureRenderer::new(&spec(DsKind::BTree, root), Theme::new(BoxStyle::Sharp))
                .render(false)
                .unwrap();
        let box_line = out.lines().find(|l| l.contains("│ 10 │ 20 │")).unwrap();
        let left_col = box_line.chars().position(|c| c == '│').unwrap();
        assert!(
            left_col > 0,
            "root box left border indented, got col {left_col}"
        );
        let box_center = left_col + "│ 10 │ 20 │".chars().count() / 2;
        let bar = out.lines().find(|l| l.contains('┴')).unwrap();
        let tee_col = bar.chars().position(|c| c == '┴').unwrap();
        assert_eq!(tee_col, box_center, "branch bar tee at box center");
    }

    #[test]
    fn test_btree_aligned_child_renders_cross_junction() {
        // When a child's center column equals the parent's descender column,
        // the branch bar renders a 4-way cross glyph instead of a tee-down.
        let root = DsNode {
            keys: vec!["10".into(), "20".into()],
            children: vec![
                DsNode {
                    keys: vec!["5".into()],
                    ..DsNode::default()
                },
                DsNode {
                    keys: vec!["15".into()],
                    ..DsNode::default()
                },
                DsNode {
                    keys: vec!["25".into()],
                    ..DsNode::default()
                },
            ],
            ..DsNode::default()
        };

        // Rounded style: ┼
        let rounded =
            DataStructureRenderer::new(&spec(DsKind::BTree, root.clone()), Theme::default())
                .render(false)
                .unwrap();
        assert!(
            rounded.contains('┼'),
            "aligned child must render cross junction: {rounded}"
        );
        assert_eq!(
            rounded.matches('┬').count(),
            0,
            "no tee-down should appear when child aligned: {rounded}"
        );

        // Ascii style: +
        let ascii = DataStructureRenderer::new(&spec(DsKind::BTree, root.clone()), Theme::ascii())
            .render(false)
            .unwrap();
        assert!(
            ascii.contains("+---------+---------+"),
            "ascii cross junction: {ascii}"
        );

        // Double style: ╬
        let double = DataStructureRenderer::new(
            &spec(DsKind::BTree, root.clone()),
            Theme::new(BoxStyle::Double),
        )
        .render(false)
        .unwrap();
        assert!(
            double.contains('╬'),
            "double style cross junction: {double}"
        );

        // Heavy style: ╋
        let heavy =
            DataStructureRenderer::new(&spec(DsKind::BTree, root), Theme::new(BoxStyle::Heavy))
                .render(false)
                .unwrap();
        assert!(heavy.contains('╋'), "heavy style cross junction: {heavy}");
    }

    #[test]
    fn test_heap_over_node_count_errors_cleanly() {
        let spec = DataStructureSpec {
            style: BoxStyle::Sharp,
            kind: DsKind::Heap,
            values: (0..2049).map(|i| i.to_string()).collect(),
            ..DataStructureSpec::default()
        };
        let err = DataStructureRenderer::new(&spec, Theme::new(BoxStyle::Sharp))
            .render(false)
            .unwrap_err();
        assert!(
            err.contains("maximum node count (2048)"),
            "clean count-based error: {err}"
        );
    }

    #[test]
    fn test_heap_at_node_count_limit_renders() {
        let spec = DataStructureSpec {
            style: BoxStyle::Sharp,
            kind: DsKind::Heap,
            values: (0..2048).map(|i| i.to_string()).collect(),
            ..DataStructureSpec::default()
        };
        let out = DataStructureRenderer::new(&spec, Theme::new(BoxStyle::Sharp))
            .render(false)
            .unwrap();
        assert!(out.contains("│ 0 │"), "boundary heap renders");
    }

    const STYLES: [BoxStyle; 5] = [
        BoxStyle::Rounded,
        BoxStyle::Sharp,
        BoxStyle::Double,
        BoxStyle::Heavy,
        BoxStyle::Ascii,
    ];

    fn node_position(out: &str, theme: &Theme, raw: &str) -> (usize, usize) {
        let label = DisplayLabel::new(raw);
        let needle = format!("{} {} ", theme.vertical_line(), label.text);
        out.lines()
            .enumerate()
            .find_map(|(row, line)| {
                line.find(&needle).map(|offset| {
                    (
                        row,
                        UnicodeWidthStr::width(&line[..offset]) + (label.width.max(1) + 4) / 2,
                    )
                })
            })
            .unwrap_or_else(|| panic!("missing {raw:?}: {out}"))
    }

    #[test]
    fn binary_unary_side_and_connectors_survive_unequal_widths() {
        for style in STYLES {
            let theme = Theme::new(style);
            for (parent, child) in [("P", "C"), ("parent-wide", "中"), ("P", "wide-child🚀")] {
                let mut outputs = Vec::new();
                for left in [true, false] {
                    let mut root = DsNode::leaf(parent);
                    root.color = Some(Color::Red);
                    if left {
                        root.left = Some(Box::new(DsNode::leaf(child)));
                    } else {
                        root.right = Some(Box::new(DsNode::leaf(child)));
                    }
                    let tree = spec(DsKind::Tree, root);
                    let renderer = DataStructureRenderer::new(&tree, theme.clone());
                    let out = renderer.render(false).unwrap();
                    let (pr, pc) = node_position(&out, &theme, parent);
                    let (cr, cc) = node_position(&out, &theme, child);
                    assert_eq!(cr - pr, 6, "{out}");
                    assert_eq!(cc < pc, left, "{out}");
                    assert_ne!(cc, pc, "{out}");
                    for (row, col) in [(pr + 2, pc), (cr - 2, cc)] {
                        assert_eq!(
                            out.lines().nth(row).unwrap().chars().nth(col),
                            Some(theme.vertical_line()),
                            "connector must meet its node center: {out}"
                        );
                    }
                    assert_eq!(Color::strip_ansi(&renderer.render(true).unwrap()), out);
                    assert_eq!(
                        out.matches(&format!("{} ", theme.vertical_line())).count(),
                        2
                    );
                    outputs.push(out);
                }
                assert_ne!(outputs[0], outputs[1]);
            }
        }
    }

    #[test]
    fn binary_ranks_and_sides_follow_heap_topology() {
        for style in STYLES {
            let theme = Theme::new(style);
            for count in [2, 6, 10] {
                let values: Vec<String> = (0..count)
                    .map(|i| format!("node{i}{}", "中".repeat(i % 3)))
                    .collect();
                let heap = DataStructureSpec {
                    kind: DsKind::Heap,
                    values,
                    ..DataStructureSpec::default()
                };
                let out = DataStructureRenderer::new(&heap, theme.clone())
                    .render(false)
                    .unwrap();
                for (i, value) in heap.values.iter().enumerate().skip(1) {
                    let parent = &heap.values[(i - 1) / 2];
                    let (pr, pc) = node_position(&out, &theme, parent);
                    let (cr, cc) = node_position(&out, &theme, value);
                    assert_eq!(cr, 1 + 6 * (i + 1).ilog2() as usize, "{out}");
                    assert_eq!(cr - pr, 6, "{out}");
                    assert_eq!(cc < pc, i % 2 == 1, "{out}");
                    assert_ne!(cc, pc, "{out}");
                }
            }
        }
    }

    #[test]
    fn alternating_binary_chain_has_no_phantom_subtrees() {
        let mut root = DsNode::leaf("n32");
        for i in (0..32).rev() {
            let mut parent = DsNode::leaf(format!("n{i}"));
            if i % 2 == 0 {
                parent.left = Some(Box::new(root));
            } else {
                parent.right = Some(Box::new(root));
            }
            root = parent;
        }
        let tree = spec(DsKind::Tree, root);
        let theme = Theme::ascii();
        let out = DataStructureRenderer::new(&tree, theme.clone())
            .render(false)
            .unwrap();
        assert_eq!(out.lines().count(), 33 * 6 - 3);
        assert!(out.lines().map(str::len).max().unwrap() < 20);
        for i in 0..32 {
            let (pr, pc) = node_position(&out, &theme, &format!("n{i}"));
            let (cr, cc) = node_position(&out, &theme, &format!("n{}", i + 1));
            assert_eq!(cr - pr, 6);
            assert_eq!(cc < pc, i % 2 == 0);
        }
    }

    #[test]
    fn integral_ordering_is_exact_and_retains_raw_topology() {
        for values in [
            ["9007199254740993", "9007199254740992", "0"],
            [
                "-9007199254740992",
                "-9007199254740993",
                "-999999999999999999999999",
            ],
        ] {
            let strings: Vec<_> = values.iter().map(|s| (*s).to_string()).collect();
            let root = build_bst(&strings).unwrap();
            let mut node = &root;
            for (i, raw) in values.iter().enumerate() {
                assert_eq!(node.value, *raw);
                assert!(node.right.is_none());
                if i + 1 < values.len() {
                    node = node.left.as_deref().unwrap();
                } else {
                    assert!(node.left.is_none());
                }
            }
            let reverse: Vec<_> = strings.into_iter().rev().collect();
            let root = build_bst(&reverse).unwrap();
            assert!(root.left.is_none());
            assert_eq!(
                root.right.as_ref().unwrap().right.as_ref().unwrap().value,
                values[0]
            );
        }
        for equal in [
            ["2", "+0002", "02"],
            ["-0", "+000", "0"],
            ["-02", "-2", "-0002"],
            ["2", "2", "2"],
        ] {
            let values: Vec<_> = equal.iter().map(|s| (*s).to_string()).collect();
            let root = build_bst(&values).unwrap();
            assert_eq!(root.value, equal[0]);
            assert!(root.left.is_none());
            let child = root.right.as_ref().unwrap();
            assert_eq!(child.value, equal[1]);
            assert!(child.left.is_none());
            assert_eq!(child.right.as_ref().unwrap().value, equal[2]);
        }
        for (a, b) in [
            ("2.5", "10.25"),
            ("-1.5", "-1"),
            ("apple", "pear"),
            ("1e2", "101"),
        ] {
            assert!(less_than(a, b));
            assert!(!less_than(b, a));
        }
        let json: DataStructureSpec = serde_json::from_str(
            r#"{"kind":"tree","values":[9007199254740993,9007199254740992,0]}"#,
        )
        .unwrap();
        let root = build_bst(&json.values).unwrap();
        assert_eq!(root.left.as_ref().unwrap().value, "9007199254740992");
        assert_eq!(
            root.left.as_ref().unwrap().left.as_ref().unwrap().value,
            "0"
        );
    }

    #[test]
    fn display_labels_borrow_plain_and_roundtrip_every_control() {
        for raw in ["", "plain", "中🚀", "a b"] {
            let label = DisplayLabel::new(raw);
            assert!(matches!(label.text, Cow::Borrowed(_)));
            assert_eq!(label.text, raw);
            assert_eq!(label.width, UnicodeWidthStr::width(raw));
        }
        let mut special = vec![
            " a ".to_string(),
            "\"quoted\"".to_string(),
            "a\\nb".to_string(),
            "a\nb".to_string(),
            "a\\tb".to_string(),
            "\u{2003}x".to_string(),
        ];
        special.extend(
            (0..=0x9f)
                .filter_map(char::from_u32)
                .filter(|c| c.is_control())
                .map(|c| format!("a{c}b")),
        );
        for raw in special {
            let label = DisplayLabel::new(&raw);
            assert!(
                !label.text.chars().any(char::is_control),
                "{:?}",
                label.text
            );
            assert_eq!(serde_json::from_str::<String>(&label.text).unwrap(), raw);
            assert_eq!(label.width, UnicodeWidthStr::width(label.text.as_ref()));
        }
        assert_ne!(
            DisplayLabel::new("a\nb").text,
            DisplayLabel::new("a\\nb").text
        );
        assert_ne!(DisplayLabel::new("\t").text, DisplayLabel::new("\\t").text);
    }

    #[test]
    fn special_labels_reach_every_native_cell_and_endpoint_once() {
        let raw = " a\nb\t\r\u{1b}\u{7f}\u{85}\\\"中🚀 ";
        let encoded = DisplayLabel::new(raw);
        for style in STYLES {
            for kind in [
                DsKind::Tree,
                DsKind::BTree,
                DsKind::Heap,
                DsKind::LinkedList,
                DsKind::DoublyLinkedList,
                DsKind::Array,
                DsKind::Queue,
                DsKind::Graph,
            ] {
                let spec = DataStructureSpec {
                    kind,
                    nodes: vec![raw.into(), "other".into()],
                    values: vec![raw.into(), "other".into()],
                    btree_root: Some(DsNode {
                        keys: vec![raw.into(), "other".into()],
                        ..DsNode::default()
                    }),
                    edges: vec![vec![raw.into(), "other".into()]],
                    head_label: Some(raw.into()),
                    tail_label: Some(raw.into()),
                    front_label: Some(raw.into()),
                    rear_label: Some(raw.into()),
                    ..DataStructureSpec::default()
                };
                let renderer = DataStructureRenderer::new(&spec, Theme::new(style));
                let out = renderer.render(false).unwrap();
                assert!(out.contains(encoded.text.as_ref()), "{kind:?}: {out}");
                assert!(!out.chars().any(|c| c.is_control() && c != '\n'));
                assert_eq!(
                    out.matches(encoded.text.as_ref()).count(),
                    match kind {
                        DsKind::LinkedList => 2,
                        DsKind::Queue | DsKind::DoublyLinkedList => 3,
                        _ => 1,
                    },
                    "{out}"
                );
                assert_eq!(
                    out.lines().count(),
                    match kind {
                        DsKind::Array => 4,
                        DsKind::Graph => 7,
                        DsKind::Tree | DsKind::Heap => 9,
                        _ => 3,
                    },
                    "{out}"
                );
                assert_eq!(renderer.render(true).unwrap(), out);
                match kind {
                    DsKind::LinkedList => assert_linear_frames(&out, &Theme::new(style), 2),
                    DsKind::Queue => assert_linear_frames(&out, &Theme::new(style), 1),
                    DsKind::DoublyLinkedList => assert_linear_frames(&out, &Theme::new(style), 3),
                    _ => {}
                }
            }
        }
    }

    fn assert_linear_frames(out: &str, theme: &Theme, cells_per_box: usize) {
        let lines: Vec<_> = out.lines().collect();
        let columns = |line: &str, a: char, b: char| {
            line.char_indices()
                .filter(|(_, ch)| *ch == a || *ch == b)
                .map(|(offset, _)| UnicodeWidthStr::width(&line[..offset]))
                .collect::<Vec<_>>()
        };
        let walls = columns(lines[1], theme.vertical_line(), theme.vertical_line());
        let borders: Vec<_> = walls
            .chunks_exact(cells_per_box + 1)
            .flat_map(|cell| [cell[0], cell[cells_per_box]])
            .collect();
        assert_eq!(walls.len() % (cells_per_box + 1), 0);
        assert_eq!(
            columns(lines[0], theme.top_left_corner(), theme.top_right_corner()),
            borders,
            "{out}"
        );
        assert_eq!(
            columns(
                lines[2],
                theme.bottom_left_corner(),
                theme.bottom_right_corner()
            ),
            borders,
            "{out}"
        );
    }

    #[test]
    fn array_ruler_uses_displayed_widths_through_index_100() {
        let values: Vec<_> = (0..=100)
            .map(|i| {
                if i % 10 == 0 {
                    "中\n🚀".into()
                } else {
                    "".into()
                }
            })
            .collect();
        let spec = DataStructureSpec {
            kind: DsKind::Array,
            values,
            ..DataStructureSpec::default()
        };
        for style in STYLES {
            let theme = Theme::new(style);
            let out = DataStructureRenderer::new(&spec, theme.clone())
                .render(false)
                .unwrap();
            let lines: Vec<_> = out.lines().collect();
            assert_eq!(lines.len(), 4);
            let frame_w = UnicodeWidthStr::width(lines[1]);
            assert_eq!(frame_w, UnicodeWidthStr::width(lines[2]));
            assert_eq!(frame_w, UnicodeWidthStr::width(lines[3]));
            let mut column = 1;
            for (i, cell) in lines[2]
                .split(theme.vertical_line())
                .skip(1)
                .take(101)
                .enumerate()
            {
                let inner_width = UnicodeWidthStr::width(cell);
                let index = i.to_string();
                let start = column + 1 + (inner_width - 2 - index.len()) / 2;
                assert_eq!(&lines[0][start..start + index.len()], index);
                column += inner_width + 1;
            }
        }
    }

    #[test]
    fn btree_color_never_enters_geometry() {
        for style in STYLES {
            for count in 1..=3 {
                let root = DsNode {
                    keys: ["10", "20", "30"][..count]
                        .iter()
                        .map(|s| (*s).into())
                        .collect(),
                    color: Some(Color::Red),
                    ..DsNode::default()
                };
                let mut tree = spec(DsKind::BTree, root);
                let renderer = DataStructureRenderer::new(&tree, Theme::new(style));
                let plain = renderer.render(false).unwrap();
                assert_eq!(Color::strip_ansi(&renderer.render(true).unwrap()), plain);
                assert_eq!(
                    UnicodeWidthStr::width(plain.lines().next().unwrap()),
                    count * 5 + 1
                );
                tree.btree_root.as_mut().unwrap().children = vec![
                    DsNode {
                        keys: vec!["a\nb".into(), "中".into()],
                        ..DsNode::default()
                    },
                    DsNode {
                        keys: vec!["x".into(), "y".into(), "z".into()],
                        color: Some(Color::Blue),
                        ..DsNode::default()
                    },
                ];
                let renderer = DataStructureRenderer::new(&tree, Theme::new(style));
                let colored = renderer.render(true).unwrap();
                assert!(colored.contains("\u{1b}[31m") && colored.contains("\u{1b}[34m"));
                assert_eq!(Color::strip_ansi(&colored), renderer.render(false).unwrap());
            }
        }
    }

    #[test]
    fn graph_rejects_coerced_duplicates_without_collapsing_edges() {
        for json in [
            r#"{"kind":"graph","nodes":["a","a","b"]}"#,
            r#"{"kind":"graph","nodes":[1,"1",2]}"#,
        ] {
            let spec: DataStructureSpec = serde_json::from_str(json).unwrap();
            let err = DataStructureRenderer::new(&spec, Theme::ascii())
                .render(false)
                .unwrap_err();
            assert!(err.contains("duplicate"));
            assert!(err.contains(&spec.nodes[0]));
        }
        let spec = graph_spec(&["a", "b"], &[&["a", "b"], &["a", "b"], &["a", "a"]]);
        let out = DataStructureRenderer::new(&spec, Theme::ascii())
            .render(false)
            .unwrap();
        assert_eq!(
            out.lines().nth(1).unwrap(),
            "| a | -> | b | -> | b | -> | a |"
        );
        assert_eq!(out.lines().nth(5).unwrap(), "| b |");
    }

    #[test]
    fn graph_whitespace_and_escape_keys_keep_raw_identity() {
        let nodes = ["", " ", "a", "a ", "a\nb", "a\\nb", "\"a \""];
        let spec = graph_spec(
            &nodes,
            &[
                &["a", "a "],
                &["a ", " "],
                &["", "a\nb"],
                &["a\nb", "a\\nb"],
                &["a\\nb", "\"a \""],
            ],
        );
        let targets = [Some(4), None, Some(3), Some(1), Some(5), Some(6), None];
        for style in STYLES {
            let theme = Theme::new(style);
            let out = DataStructureRenderer::new(&spec, theme.clone())
                .render(false)
                .unwrap();
            let lines: Vec<_> = out.lines().collect();
            assert_eq!(lines.len(), 27);
            for (i, target) in targets.iter().enumerate() {
                let fields: Vec<_> = lines[i * 4 + 1]
                    .split(theme.vertical_line())
                    .skip(1)
                    .step_by(2)
                    .map(str::trim)
                    .collect();
                assert_eq!(fields[0], DisplayLabel::new(nodes[i]).text);
                assert_eq!(fields.len(), 1 + usize::from(target.is_some()));
                if let Some(target) = target {
                    assert_eq!(fields[1], DisplayLabel::new(nodes[*target]).text);
                }
                let width = UnicodeWidthStr::width(lines[i * 4]);
                assert_eq!(width, UnicodeWidthStr::width(lines[i * 4 + 1]));
                assert_eq!(width, UnicodeWidthStr::width(lines[i * 4 + 2]));
            }
        }
    }

    #[test]
    fn doublylinkedlist_pointer_cells_and_endpoints_follow_occurrences() {
        for nodes in [
            vec!["one"],
            vec!["10", "20", "30"],
            vec!["dup", "dup", "0", "", "NULL"],
            vec!["long-value", "中🚀", "a\nb", "a\\nb"],
        ] {
            for style in STYLES {
                let theme = Theme::new(style);
                let spec = DataStructureSpec {
                    kind: DsKind::DoublyLinkedList,
                    nodes: nodes.iter().map(|s| (*s).into()).collect(),
                    head_label: Some(" h\t".into()),
                    tail_label: Some("t\r ".into()),
                    ..DataStructureSpec::default()
                };
                let renderer = DataStructureRenderer::new(&spec, theme.clone());
                let out = renderer.render(false).unwrap();
                assert_eq!(out, renderer.render(true).unwrap());
                let lines: Vec<_> = out.lines().collect();
                assert_eq!(lines.len(), 3);
                let fields: Vec<_> = lines[1].split(theme.vertical_line()).collect();
                assert_eq!(fields.len(), 4 * nodes.len() + 1);
                assert_eq!(
                    fields[0],
                    format!(
                        "{} {}{} ",
                        DisplayLabel::new(" h\t").text,
                        theme.horizontal_line(),
                        theme.arrow_right()
                    )
                );
                assert_eq!(
                    fields.last().unwrap(),
                    &format!(
                        " {}{} {}",
                        theme.arrow_left(),
                        theme.horizontal_line(),
                        DisplayLabel::new("t\r ").text
                    )
                );
                let (pointer, null) = if style == BoxStyle::Ascii {
                    ("X", "NULL")
                } else {
                    ("●", "∅")
                };
                for (i, raw) in nodes.iter().enumerate() {
                    assert_eq!(
                        fields[i * 4 + 1].trim(),
                        if i == 0 { null } else { pointer }
                    );
                    assert_eq!(fields[i * 4 + 2].trim(), DisplayLabel::new(raw).text);
                    assert_eq!(
                        fields[i * 4 + 3].trim(),
                        if i + 1 == nodes.len() { null } else { pointer }
                    );
                    if i > 0 {
                        assert_eq!(
                            fields[i * 4],
                            format!(
                                " {}{}{} ",
                                theme.arrow_left(),
                                theme.horizontal_line(),
                                theme.arrow_right()
                            )
                        );
                    }
                }
                assert_eq!(
                    UnicodeWidthStr::width(lines[0]),
                    UnicodeWidthStr::width(lines[2])
                );
                assert_linear_frames(&out, &theme, 3);
                if style == BoxStyle::Ascii && nodes.iter().all(|s| s.is_ascii()) {
                    assert!(out.is_ascii());
                }
            }
        }
        let empty = DataStructureSpec {
            kind: DsKind::DoublyLinkedList,
            ..DataStructureSpec::default()
        };
        let err = DataStructureRenderer::new(&empty, Theme::ascii())
            .render(false)
            .unwrap_err();
        assert!(err.contains("doublylinkedlist") && err.contains("nodes"));
    }
}
