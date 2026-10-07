use crate::color::Color;
use crate::schema::{DataStructureSpec, DsKind, DsNode};
use crate::theme::{BoxStyle, Theme};
use unicode_width::UnicodeWidthStr;

/// Paints `s` when `colored` and a color is set; otherwise returns it plain.
fn paint(colored: bool, color: Option<Color>, s: &str) -> String {
    match (colored, color) {
        (true, Some(c)) => c.paint(s),
        _ => s.to_string(),
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

/// Renders textbook-style data-structure diagrams: binary trees
/// (`kind: "tree"`), B-trees (`kind: "btree"`), linked lists
/// (`kind: "linkedlist"`), and arrays (`kind: "array"`).
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
    /// `root` or `values`, `btree` needs `btree_root`, `linkedlist` needs
    /// `nodes`, and `array` needs `values`.
    pub fn render(&self, colored: bool) -> Result<String, String> {
        let (mut lines, width) = match self.spec.kind {
            DsKind::LinkedList => self.render_linkedlist()?,
            DsKind::Array => self.render_array()?,
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
            DsKind::Tree => [node.left.as_deref(), node.right.as_deref()]
                .iter()
                .flatten()
                .copied()
                .collect(),
            DsKind::BTree => node.children.iter().collect(),
            // Linear kinds render their own single-row layouts.
            DsKind::LinkedList | DsKind::Array => Vec::new(),
        };
        // Recurse in a slim stack frame: debug-build frames for the full
        // layout body are far too wide to stack 2048 deep, so the recursion
        // lives in `render_children` and assembly runs in `assemble_block`.
        let child_blocks = self.render_children(&child_nodes, effective, colored, depth)?;
        Ok(self.assemble_block(effective, colored, box_lines, box_w, child_blocks))
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
        let sub_w = box_w.max(children_total);
        let child_start = (sub_w - children_total) / 2;
        let box_x = (sub_w - box_w) / 2;
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

        // Connector rows. A single aligned child is one `│` row; any other
        // shape gets a descender, a branch bar, and child descenders.
        if centers.len() == 1 && centers[0] == parent_center {
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
        let head = self.spec.head_label.as_deref().unwrap_or("head");
        let is_ascii = self.theme.box_style == BoxStyle::Ascii;
        let (pointer, null) = if is_ascii {
            ("X", "NULL")
        } else {
            ("●", "∅")
        };

        // Uniform value-cell width keeps the chain boxes visually level.
        let value_w = self
            .spec
            .nodes
            .iter()
            .map(|v| UnicodeWidthStr::width(v.as_str()))
            .max()
            .unwrap_or(1)
            .max(1);
        let boxes: Vec<(Vec<String>, usize)> = self
            .spec
            .nodes
            .iter()
            .enumerate()
            .map(|(i, v)| {
                let ptr = if i + 1 == self.spec.nodes.len() {
                    null
                } else {
                    pointer
                };
                let ws = [value_w, UnicodeWidthStr::width(ptr).max(1)];
                self.cells_box_widths(&[v.as_str(), ptr], &ws, None, false)
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
        let pad = " ".repeat(UnicodeWidthStr::width(head) + join_w);

        let mut top = pad.clone();
        let mut mid = format!("{head}{conn}");
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

    /// Renders `kind: "array"`: one boxed row of cells with separators and
    /// a centered index ruler row above.
    fn render_array(&self) -> Result<(Vec<String>, usize), String> {
        let values = &self.spec.values;
        if values.is_empty() {
            return Err("array diagram needs a `values` list".to_string());
        }
        // Per-cell width: value, index, or one column — whichever is
        // widest — so each ruler index centers over its cell.
        let widths: Vec<usize> = values
            .iter()
            .enumerate()
            .map(|(i, v)| {
                UnicodeWidthStr::width(v.as_str())
                    .max(UnicodeWidthStr::width(i.to_string().as_str()))
                    .max(1)
            })
            .collect();
        let cells: Vec<&str> = values.iter().map(String::as_str).collect();
        let (box_lines, _) = self.cells_box_widths(&cells, &widths, None, false);

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

    /// Builds the 3-line node box: `┌────┐ / │ 10 │ 20 │ / └────┘`. Binary
    /// nodes render their single `value`; B-tree nodes render `keys` cells
    /// separated by the theme's vertical glyph. Border glyphs (corners,
    /// bars, cell separators) take `color`; label text stays default.
    fn node_box(&self, node: &DsNode, color: Option<Color>, colored: bool) -> (Vec<String>, usize) {
        let cells: Vec<&str> = match self.spec.kind {
            DsKind::Tree => vec![&node.value],
            // B-tree nodes render `keys`; a keyless node falls back to its
            // `value` so `DsNode::leaf("5")` still renders a labeled cell.
            DsKind::BTree if !node.keys.is_empty() => {
                node.keys.iter().map(String::as_str).collect()
            }
            DsKind::BTree if node.value.is_empty() => vec![""],
            DsKind::BTree => vec![&node.value],
            // Linear kinds build their boxes in their own render fns.
            DsKind::LinkedList | DsKind::Array => vec![&node.value],
        };
        let cell_w = cells
            .iter()
            .map(|c| UnicodeWidthStr::width(*c))
            .max()
            .unwrap_or(0)
            .max(1);
        let ws = vec![cell_w; cells.len()];
        self.cells_box_widths(&cells, &ws, color, colored)
    }

    /// Builds the 3-line box around `cells` laid out at the per-cell widths
    /// in `widths` (parallel to `cells`), separated by the theme's vertical
    /// glyph: `┌──┬──┐ / │ c1 │ c2 │ / └──┴──┘`. Border glyphs (corners,
    /// bars, separators) are painted `color` when `colored`; cell text is
    /// never painted, so labels stay terminal-default.
    fn cells_box_widths(
        &self,
        cells: &[&str],
        widths: &[usize],
        color: Option<Color>,
        colored: bool,
    ) -> (Vec<String>, usize) {
        let sep = paint(colored, color, &self.theme.vertical_line().to_string());
        let inner = cells
            .iter()
            .zip(widths)
            .map(|(c, &w)| {
                let pad = w.saturating_sub(UnicodeWidthStr::width(*c));
                format!(" {c}{} ", " ".repeat(pad))
            })
            .collect::<Vec<_>>()
            .join(&sep);

        let (tl, tr, bl, br, h) = (
            self.theme.top_left_corner(),
            self.theme.top_right_corner(),
            self.theme.bottom_left_corner(),
            self.theme.bottom_right_corner(),
            self.theme.horizontal_line(),
        );
        let box_w = UnicodeWidthStr::width(inner.as_str()) + 2;
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
    match (a.parse::<f64>(), b.parse::<f64>()) {
        (Ok(x), Ok(y)) => x < y,
        _ => a < b,
    }
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
    fn test_tree_root_box_centered_over_children() {
        // values ["2","1","3"]: two leaf children (w 5 each, gap 4) give
        // sub_w 14, so box_x = (14-5)/2 = 4 and the parent center (box col 6)
        // must equal the descender column.
        let spec = DataStructureSpec {
            style: BoxStyle::Sharp,
            kind: DsKind::Tree,
            values: vec!["2".into(), "1".into(), "3".into()],
            ..DataStructureSpec::default()
        };
        let out = DataStructureRenderer::new(&spec, Theme::ascii())
            .render(false)
            .unwrap();
        let box_idx = out.lines().position(|l| l.contains("| 2 |")).unwrap();
        let box_line = out.lines().nth(box_idx).unwrap();
        assert_eq!(
            box_line.trim_end(),
            "    | 2 |",
            "root box indented by box_x=4"
        );
        let descender = out
            .lines()
            .skip(box_idx + 1) // past the box rows (they contain `|` too)
            .find(|l| l.contains('|'))
            .unwrap();
        let descender_col = descender.chars().position(|c| c == '|').unwrap();
        let box_center = box_line.chars().position(|c| c == '2').unwrap();
        assert_eq!(descender_col, 6, "descender under box center");
        assert_eq!(descender_col, box_center, "descender under box center");
    }

    #[test]
    fn test_wide_child_centers_parent_box() {
        // Single wider child "333" (w 7): parent box "| 2 |" (w 5) must be
        // centered at the child's center so the connector lines up.
        let root = DsNode {
            value: "2".into(),
            right: Some(Box::new(DsNode::leaf("333"))),
            ..DsNode::default()
        };
        let out = DataStructureRenderer::new(&spec(DsKind::Tree, root), Theme::ascii())
            .render(false)
            .unwrap();
        let box_idx = out.lines().position(|l| l.contains("| 2 |")).unwrap();
        let box_line = out.lines().nth(box_idx).unwrap();
        let box_center = box_line.chars().position(|c| c == '2').unwrap();
        let descender = out
            .lines()
            .skip(box_idx + 1) // past the box rows (they contain `|` too)
            .find(|l| l.contains('|'))
            .unwrap();
        let descender_col = descender.chars().position(|c| c == '|').unwrap();
        assert!(
            box_line.starts_with(' '),
            "parent box indented over wider child"
        );
        assert_eq!(descender_col, box_center, "connector meets box center");
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
}
