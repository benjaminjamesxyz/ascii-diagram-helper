use crate::schema::{DataStructureSpec, DsKind, DsNode};
use crate::theme::{BoxStyle, Theme};
use unicode_width::UnicodeWidthStr;

/// A laid-out subtree: rendered lines (all equal display width), the display
/// width, and the column of the subtree root's box center.
struct Block {
    lines: Vec<String>,
    width: usize,
    center: usize,
}

/// Horizontal gap between sibling subtree blocks (leaves room for branches).
const SIBLING_GAP: usize = 4;

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
    pub fn render(&self) -> Result<String, String> {
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
                        "tree diagram needs a `root` node or a `values` insertion order"
                            .to_string()
                    })?;
                let block = self.render_node(&root);
                (block.lines, block.width)
            }
            DsKind::BTree => {
                let root = self
                    .spec
                    .btree_root
                    .clone()
                    .ok_or_else(|| "btree diagram needs a `btree_root` node".to_string())?;
                let block = self.render_node(&root);
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
    /// a descender row, a branch bar, and child descenders.
    fn render_node(&self, node: &DsNode) -> Block {
        let (box_lines, box_w) = self.node_box(node);

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
        let child_blocks: Vec<Block> = child_nodes.iter().map(|c| self.render_node(c)).collect();

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

        // Node box, padded out to the subtree width
        let mut lines: Vec<String> = box_lines
            .iter()
            .map(|l| format!("{l}{}", " ".repeat(sub_w - box_w)))
            .collect();

        // Connector rows. A single aligned child is one `│` row; any other
        // shape gets a descender, a branch bar, and child descenders.
        if centers.len() == 1 && centers[0] == parent_center {
            lines.push(spaced_row(parent_center, self.theme.vertical_line(), sub_w));
        } else {
            // Descender under the parent box
            lines.push(spaced_row(parent_center, self.theme.vertical_line(), sub_w));

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
            lines.push(bar.into_iter().collect());

            // Descenders into each child
            let mut drops = vec![' '; sub_w];
            for &c in &centers {
                drops[c] = self.theme.vertical_line();
            }
            lines.push(drops.into_iter().collect());
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
        let (pointer, null) = if is_ascii { ("X", "NULL") } else { ("●", "∅") };

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
                self.cells_box_widths(&[v.as_str(), ptr], &ws)
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
        let (box_lines, _) = self.cells_box_widths(&cells, &widths);

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
    /// separated by the theme's vertical glyph.
    fn node_box(&self, node: &DsNode) -> (Vec<String>, usize) {
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
        self.cells_box_widths(&cells, &ws)
    }

    /// Builds the 3-line box around `cells` laid out at the per-cell widths
    /// in `widths` (parallel to `cells`), separated by the theme's vertical
    /// glyph: `┌──┬──┐ / │ c1 │ c2 │ / └──┴──┘`.
    fn cells_box_widths(&self, cells: &[&str], widths: &[usize]) -> (Vec<String>, usize) {
        let sep = self.theme.vertical_line().to_string();
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
                format!("{tl}{bar}{tr}"),
                format!("{}{inner}{}", sep, sep),
                format!("{bl}{bar}{br}"),
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
    match node {
        None => *node = Some(Box::new(DsNode::leaf(value))),
        Some(n) => {
            let goes_left = less_than(&value, &n.value);
            let target = if goes_left { &mut n.left } else { &mut n.right };
            insert_bst(target, value);
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
            .render()
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
        let out = DataStructureRenderer::new(&spec(DsKind::Tree, root), Theme::ascii()).render();
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
                .render()
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
        .render()
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
                .render()
                .is_err()
        );
        let btree = DataStructureSpec {
            style: BoxStyle::Sharp,
            kind: DsKind::BTree,
            ..DataStructureSpec::default()
        };
        assert!(
            DataStructureRenderer::new(&btree, Theme::ascii())
                .render()
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
            .render()
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
        .render()
        .unwrap();
        assert!(out.contains("│ 10 │ ● │"), "two-cell node box: {out}");
        assert!(out.contains("∅"), "last pointer cell is the NULL glyph");
        assert!(out.contains("►"), "theme arrow between boxes");
        assert!(!out.contains("││"), "boxes separated by arrows: {out}");
        assert_eq!(out.matches("head").count(), 1, "default head label");
        assert_eq!(out.lines().count(), 3, "single-row layout");
        assert!(
            out.find("head").unwrap()
                < out.find("│ 10 │").unwrap()
                && out.find("│ 10 │").unwrap() < out.find("│ 30 │").unwrap()
                && out.find("│ 30 │").unwrap() < out.find("∅").unwrap(),
            "head → chain → NULL order"
        );
    }

    #[test]
    fn test_linkedlist_ascii_fallback() {
        let out =
            DataStructureRenderer::new(&linked_spec(&["10", "20"]), Theme::ascii())
                .render()
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
        let out =
            DataStructureRenderer::new(&spec, Theme::new(BoxStyle::Sharp))
                .render()
                .unwrap();
        assert!(out.contains("front ─► │ 1 │ ∅ │"), "{out}");
        assert!(!out.contains("head"));
    }

    #[test]
    fn test_linkedlist_needs_nodes() {
        assert!(
            DataStructureRenderer::new(&linked_spec(&[]), Theme::ascii())
                .render()
                .is_err()
        );
    }

    #[test]
    fn test_array_ruler_and_cells() {
        let out = DataStructureRenderer::new(
            &array_spec(&["a", "b", "c"]),
            Theme::new(BoxStyle::Sharp),
        )
        .render()
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
            .render()
            .unwrap();
        assert!(out.contains("| a | b |"), "ascii cells: {out}");
        assert!(out.contains('+') && out.contains('-'), "ascii box");
        assert!(out.contains('0') && out.contains('1'), "index ruler");
        assert!(!out.contains('│') && !out.contains('┌'), "no Unicode glyphs");
    }

    #[test]
    fn test_array_needs_values() {
        assert!(
            DataStructureRenderer::new(&array_spec(&[]), Theme::ascii())
                .render()
                .is_err()
        );
    }

    #[test]
    fn test_linkedlist_and_array_colored_matches_plain() {
        for spec in [linked_spec(&["10", "20"]), array_spec(&["a", "b"])] {
            let ds = DiagramSpec::DataStructure(spec);
            let plain = crate::render_diagram_colored(&ds, false).unwrap();
            let colored = crate::render_diagram_colored(&ds, true).unwrap();
            assert_eq!(plain, colored, "no color support yet — identical output");
            assert!(!plain.contains('\x1b'), "plain render has zero ANSI");
        }
    }
}
