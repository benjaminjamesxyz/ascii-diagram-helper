use crate::color::Color;
use crate::schema::{
    ArrowDirection, DataStructureSpec, DiagramSpec, DsKind, DsNode, EdgeSpec, FlowchartSpec,
    LayoutDirection, NodeShape, NodeSpec, ParticipantSpec, SeqFrameSpec, SeqMessageSpec,
    SeqMessageType, SeqNotePosition, SeqNoteSpec, SequenceSpec, StackLayerSpec, StackSpec,
    SubgraphSpec, TableSpec, TextAlign, TreeNodeSpec, TreeSpec,
};
use crate::theme::BoxStyle;

/// Per-class style marks: dashed border + optional border color.
/// Deferred style from `classDef` / `class` directives.
#[derive(Clone, Copy, Default)]
struct ClassStyle {
    dashed: bool,
    stroke: Option<Color>,
    fill: Option<Color>,
    thick: u8,
}

/// Parses a numeric prop (`stroke-width:2px`) into a border weight level:
/// `0` default, `1` heavy (>= 2), `2` double (>= 3).
fn prop_border_level(props: &str, name: &str) -> u8 {
    prop_value(props, name)
        .and_then(|v| v.trim_end_matches("px").trim().parse::<f64>().ok())
        .map(|w| {
            if w >= 3.0 {
                2
            } else if w >= 2.0 {
                1
            } else {
                0
            }
        })
        .unwrap_or(0)
}

/// Extracts the value of `name` from a Mermaid prop list like
/// `fill:#eee, stroke:red` — splits on commas, each chunk at its first `:`.
fn prop_value<'a>(props: &'a str, name: &str) -> Option<&'a str> {
    props.split(',').find_map(|chunk| {
        let (k, v) = chunk.split_once(':')?;
        k.trim().eq_ignore_ascii_case(name).then_some(v.trim())
    })
}

/// True if `name` appears in the prop list, with or without a value
/// (`stroke-dasharray` is often a bare flag).
fn prop_flag(props: &str, name: &str) -> bool {
    props.split(',').any(|chunk| {
        let chunk = chunk.trim();
        match chunk.split_once(':') {
            Some((k, _)) => k.trim().eq_ignore_ascii_case(name),
            None => chunk.eq_ignore_ascii_case(name),
        }
    })
}

/// Parses a color-valued prop (`stroke:red`, `stroke:#f80`) if well-formed.
fn prop_color(props: &str, name: &str) -> Option<Color> {
    prop_value(props, name).and_then(Color::parse)
}

/// Parses any supported diagram DSL or a JSON [`DiagramSpec`].
///
/// # Errors
///
/// Returns `Err` if the input is empty, JSON input does not deserialize into
/// [`DiagramSpec`], or no diagram syntax can be detected.
pub fn parse_dsl_or_json(input: &str, default_style: BoxStyle) -> Result<DiagramSpec, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("Input diagram specification is empty".to_string());
    }

    // Check if JSON
    if trimmed.starts_with('{') {
        match serde_json::from_str::<DiagramSpec>(trimmed) {
            Ok(spec) => return Ok(spec),
            Err(e) => return Err(format!("Invalid JSON diagram specification: {e}")),
        }
    }

    // Mermaid allows `;` as a statement separator; normalize to newlines so
    // single-line inputs (CLI `dsl` mode) parse like multi-line input.
    let normalized = normalize_semicolons(trimmed);
    let trimmed = normalized.as_str();

    let first_line = trimmed.lines().next().unwrap_or("").trim();

    if first_line.starts_with("sequenceDiagram") {
        parse_sequence_dsl(trimmed, default_style)
    } else if first_line.starts_with("graph") || first_line.starts_with("flowchart") {
        parse_flowchart_dsl(trimmed, default_style)
    } else if first_line.starts_with("stack")
        || first_line.starts_with("memory")
        || first_line.starts_with("memory-map")
    {
        parse_stack_dsl(trimmed, default_style)
    } else if first_line.starts_with("table") || first_line.starts_with('|') {
        parse_table_dsl(trimmed, default_style)
    } else if first_line.starts_with("tree")
        || trimmed.lines().any(|l| l.trim_start().starts_with("- "))
    {
        parse_tree_dsl(trimmed, default_style)
    } else if first_line.starts_with("datastructure") {
        Err(
            "datastructure diagrams are JSON-only, e.g. {\"type\":\"datastructure\",\"kind\":\"tree\",\"values\":[\"8\",\"3\",\"10\",\"1\",\"6\"]} — or use the `ds` shorthand, e.g. `ds tree 8 3 10 1 6`"
                .to_string(),
        )
    } else if first_line.split_whitespace().next() == Some("ds") {
        parse_datastructure_dsl(trimmed, default_style)
    } else if trimmed.contains("-->") || trimmed.contains("->") {
        // Default to flowchart if arrow detected
        parse_flowchart_dsl(trimmed, default_style)
    } else {
        // Fallback to tree
        parse_tree_dsl(trimmed, default_style)
    }
}

/// Expected `ds` syntax, appended to every malformed-input error so the
/// message names the fix.
const DS_DSL_HINT: &str = "expected `ds tree <value...>` or \
    `ds btree <rootkeys> | <level cells> | ...`, e.g. \
    `ds tree 8 3 10 1 6` or `ds btree 10,20 | 3,5 12,15 25,30`";

/// Parses the `ds` DSL shorthand into a [`DataStructureSpec`] diagram.
///
/// Two forms are supported:
/// - `ds tree <v...>` — binary tree from insertion order (BST build), e.g.
///   `ds tree 8 3 10 1 6`
/// - `ds btree <rootkeys> | <next-level cells> | ...` — pipe-separated
///   levels; each cell is a comma-separated key list, cells within a level
///   are whitespace-separated. Children are assigned level-order (BFS): a
///   node with `n` keys consumes up to `n + 1` cells as its children, and
///   fewer cells render as-is (same as the JSON path).
///
/// # Errors
///
/// Returns `Err` naming the expected syntax when the kind is unknown or a
/// key list (root keys or a level cell) is empty.
pub fn parse_datastructure_dsl(
    input: &str,
    default_style: BoxStyle,
) -> Result<DiagramSpec, String> {
    // Drop the two leading tokens (`ds <kind>`), keeping the raw remainder
    // so `|`-separated levels keep their spacing.
    let mut tokens = input.splitn(3, char::is_whitespace);
    let _ds = tokens.next();
    let kind = tokens.next().unwrap_or_default();
    let rest = tokens.next().unwrap_or_default().trim();

    match kind {
        "tree" => {
            let values: Vec<String> = rest.split_whitespace().map(str::to_string).collect();
            if values.is_empty() {
                return Err(format!("ds tree needs at least one value; {DS_DSL_HINT}"));
            }
            Ok(DiagramSpec::DataStructure(DataStructureSpec {
                style: default_style,
                kind: DsKind::Tree,
                values,
                ..DataStructureSpec::default()
            }))
        }
        "btree" => {
            if rest.is_empty() {
                return Err(format!("ds btree needs a root key list; {DS_DSL_HINT}"));
            }
            let mut levels = rest.split('|');
            let mut root = DsNode {
                keys: parse_ds_keys(levels.next().unwrap_or_default(), "root key")?,
                ..DsNode::default()
            };

            // Flatten the remaining levels into a cell stream and attach
            // children level-order (BFS). A node with `n` keys takes up to
            // `n + 1` cells; running dry early is permissive (rendered
            // as-is), leftovers mean no parent had a free slot.
            let mut cells = levels
                .flat_map(str::split_whitespace)
                .map(|cell| parse_ds_keys(cell, "level cell"))
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .peekable();
            let mut queue: std::collections::VecDeque<&mut DsNode> =
                std::collections::VecDeque::new();
            queue.push_back(&mut root);
            while let Some(node) = queue.pop_front() {
                if cells.peek().is_none() {
                    break;
                }
                for _ in 0..=node.keys.len() {
                    let Some(keys) = cells.next() else {
                        break;
                    };
                    node.children.push(DsNode {
                        keys,
                        ..DsNode::default()
                    });
                }
                queue.extend(node.children.iter_mut());
            }
            Ok(DiagramSpec::DataStructure(DataStructureSpec {
                style: default_style,
                kind: DsKind::BTree,
                btree_root: Some(root),
                ..DataStructureSpec::default()
            }))
        }
        other => Err(format!("unknown `ds` kind `{other}`; {DS_DSL_HINT}")),
    }
}

/// Splits a comma-separated `ds btree` key list, rejecting empty keys.
fn parse_ds_keys(cell: &str, what: &str) -> Result<Vec<String>, String> {
    let keys: Vec<String> = cell.split(',').map(str::trim).map(str::to_string).collect();
    if keys.iter().any(String::is_empty) {
        return Err(format!(
            "empty {what} in `ds btree` input (`{cell}`); {DS_DSL_HINT}"
        ));
    }
    Ok(keys)
}

/// Parses a Mermaid-style flowchart DSL into a [`FlowchartSpec`].
///
/// # Errors
///
/// Currently always returns `Ok`; the `Result` keeps the parser signatures
/// uniform with the other DSL parsers.
pub fn parse_flowchart_dsl(input: &str, default_style: BoxStyle) -> Result<DiagramSpec, String> {
    let mut direction = LayoutDirection::TB;
    let mut nodes: Vec<NodeSpec> = Vec::new();
    let mut node_indices: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();
    let mut edges: Vec<EdgeSpec> = Vec::new();
    // Mermaid style directives, deferred until all nodes/edges exist
    let mut class_styles: std::collections::HashMap<String, ClassStyle> =
        std::collections::HashMap::new();
    let mut dashed_nodes: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut colored_nodes: std::collections::HashMap<String, Color> =
        std::collections::HashMap::new();
    let mut filled_nodes: std::collections::HashMap<String, Color> =
        std::collections::HashMap::new();
    let mut border_nodes: std::collections::HashMap<String, u8> = std::collections::HashMap::new();
    let mut dashed_links: std::collections::HashSet<usize> = std::collections::HashSet::new();
    let mut colored_links: std::collections::HashMap<usize, Color> =
        std::collections::HashMap::new();
    // Subgraph ids styled via `style <sg-id> stroke:<color>`; node ids may
    // also land here — build_subgraph_tree only looks up subgraph ids
    let mut colored_sgs: std::collections::HashMap<String, Color> =
        std::collections::HashMap::new();
    // Subgraph blocks: flat storage + open-stack, assembled into a tree below.
    // RefCell because `ensure_node` (a long-lived closure) records membership
    // while the main loop pushes/pops open blocks.
    struct SubgraphFlat {
        id: String,
        title: Option<String>,
        parent: Option<usize>,
        direction: Option<LayoutDirection>,
        members: Vec<String>,
        seen: std::collections::HashSet<String>,
    }
    let subgraphs_flat: std::cell::RefCell<Vec<SubgraphFlat>> = std::cell::RefCell::new(Vec::new());
    let sg_stack: std::cell::RefCell<Vec<usize>> = std::cell::RefCell::new(Vec::new());

    let mut lines = input.lines();
    let first_line = lines.next().unwrap_or("").trim();

    if first_line.starts_with("graph") || first_line.starts_with("flowchart") {
        let parts: Vec<&str> = first_line.split_whitespace().collect();
        if parts.len() > 1 {
            direction = match parts[1].to_uppercase().as_str() {
                "LR" => LayoutDirection::LR,
                "RL" => LayoutDirection::RL,
                "BT" => LayoutDirection::BT,
                _ => LayoutDirection::TB,
            };
        }
    } else {
        // Put first line back by processing all lines
        lines = input.lines();
    }

    let mut ensure_node = |id: &str, label: &str, shape: NodeShape| {
        if let Some(&idx) = node_indices.get(id) {
            if !label.is_empty() && nodes[idx].label == nodes[idx].id {
                nodes[idx].label = label.to_string();
                nodes[idx].shape = shape;
            }
        } else {
            node_indices.insert(id.to_string(), nodes.len());
            nodes.push(NodeSpec {
                id: id.to_string(),
                label: if label.is_empty() {
                    id.to_string()
                } else {
                    label.to_string()
                },
                shape,
                dashed_border: false,
                color: None,
                fill_color: None,
                border_level: 0,
            });
            // Nodes first declared while a subgraph is open become members
            if let Some(&cur) = sg_stack.borrow().last() {
                let mut flats = subgraphs_flat.borrow_mut();
                let sg = &mut flats[cur];
                if sg.seen.insert(id.to_string()) {
                    sg.members.push(id.to_string());
                }
            }
        }
    };

    for line in lines {
        let trimmed = line.trim();
        if trimmed.is_empty()
            || trimmed.starts_with("```")
            || trimmed.starts_with("%%")
            || trimmed.starts_with("//")
        {
            continue;
        }

        // direction TB|LR|RL|BT — global (first statement) or per-subgraph
        // (inside a subgraph block, applies to that subgraph's members)
        if let Some(rest) = trimmed.strip_prefix("direction") {
            let dir = match rest.trim().to_uppercase().as_str() {
                "LR" => Some(LayoutDirection::LR),
                "RL" => Some(LayoutDirection::RL),
                "BT" => Some(LayoutDirection::BT),
                "TB" => Some(LayoutDirection::TB),
                _ => None,
            };
            if let (Some(dir), Some(&cur)) = (dir, sg_stack.borrow().last()) {
                subgraphs_flat.borrow_mut()[cur].direction = Some(dir);
            } else if let Some(dir) = dir {
                // Top-level `direction` line (e.g. after `flowchart` header)
                direction = dir;
            }
            continue;
        }

        // subgraph id[title] | subgraph id | subgraph title — opens a group box
        if let Some(rest) = trimmed.strip_prefix("subgraph") {
            let rest = rest.trim();
            if rest.is_empty() {
                continue;
            }
            let (id, title) = if let Some(start) = rest.find('[')
                && let Some(end) = rest.rfind(']')
                && start < end
            {
                (
                    rest[..start].trim().to_string(),
                    Some(clean_label(&rest[start + 1..end])),
                )
            } else {
                (rest.to_string(), Some(rest.to_string()))
            };
            subgraphs_flat.borrow_mut().push(SubgraphFlat {
                id,
                title,
                parent: sg_stack.borrow().last().copied(),
                direction: None,
                members: Vec::new(),
                seen: std::collections::HashSet::new(),
            });
            let new_idx = subgraphs_flat.borrow().len() - 1;
            sg_stack.borrow_mut().push(new_idx);
            continue;
        }

        // end — closes the innermost open subgraph (plain `end` outside any
        // subgraph is ignored)
        if trimmed == "end" || trimmed.starts_with("end ") {
            sg_stack.borrow_mut().pop();
            continue;
        }

        // classDef name prop:value,... — stroke-dasharray (dashed border),
        // stroke:<name|#hex> (border color), fill:<name|#hex> (label text
        // color), stroke-width:2px (heavy) / 3px+ (double border)
        if let Some(rest) = trimmed.strip_prefix("classDef ") {
            if let Some((names, props)) = rest.split_once(char::is_whitespace) {
                let style = ClassStyle {
                    dashed: prop_flag(props, "stroke-dasharray"),
                    stroke: prop_color(props, "stroke"),
                    fill: prop_color(props, "fill"),
                    thick: prop_border_level(props, "stroke-width"),
                };
                for name in names.split(',') {
                    let name = name.trim();
                    if !name.is_empty() {
                        class_styles.insert(name.to_string(), style);
                    }
                }
            }
            continue;
        }

        // class id1,id2 className
        if let Some(rest) = trimmed.strip_prefix("class ") {
            if let Some((ids, class_name)) = rest.rsplit_once(char::is_whitespace) {
                let name = class_name.trim();
                let style = class_styles.get(name).copied().unwrap_or_default();
                for id in ids.split(',') {
                    let id = id.trim().to_string();
                    if style.dashed {
                        dashed_nodes.insert(id.clone());
                    }
                    if let Some(c) = style.stroke {
                        colored_nodes.insert(id.clone(), c);
                    }
                    if let Some(c) = style.fill {
                        filled_nodes.insert(id.clone(), c);
                    }
                    if style.thick > 0 {
                        border_nodes.insert(id.clone(), style.thick);
                    }
                }
            }
            continue;
        }

        // style id prop:value,...
        if let Some(rest) = trimmed.strip_prefix("style ") {
            if let Some((id, props)) = rest.split_once(char::is_whitespace) {
                let id = id.trim().to_string();
                if prop_flag(props, "stroke-dasharray") {
                    dashed_nodes.insert(id.clone());
                }
                if let Some(c) = prop_color(props, "stroke") {
                    colored_nodes.insert(id.clone(), c);
                    colored_sgs.insert(id.clone(), c);
                }
                if let Some(c) = prop_color(props, "fill") {
                    filled_nodes.insert(id.clone(), c);
                }
                let lvl = prop_border_level(props, "stroke-width");
                if lvl > 0 {
                    border_nodes.insert(id.clone(), lvl);
                }
            }
            continue;
        }

        // linkStyle 0,2 prop:value,... — dashed / colored edges. `fill` is
        // aliased to the edge color (Mermaid links have no fill; the terminal
        // edge color IS the line color)
        if let Some(rest) = trimmed.strip_prefix("linkStyle ") {
            if let Some((idxs, props)) = rest.split_once(char::is_whitespace) {
                let dashed = prop_flag(props, "stroke-dasharray");
                let color = prop_color(props, "stroke").or_else(|| prop_color(props, "fill"));
                for idx in idxs.split(',') {
                    if let Ok(i) = idx.trim().parse::<usize>() {
                        if dashed {
                            dashed_links.insert(i);
                        }
                        if let Some(c) = color {
                            colored_links.insert(i, c);
                        }
                    }
                }
            }
            continue;
        }

        if find_next_delim(trimmed).is_some() {
            parse_flowchart_edges_in_line(trimmed, &mut ensure_node, &mut edges)?;
        } else {
            // Standalone node definition: e.g. A[Label]. Reject tokens with
            // residue the same way edge statements are — an unrecognized
            // delimiter like `A ~>> B` must error, not absorb prose into a
            // node id.
            if !node_token_fully_consumed(trimmed) {
                return Err(format!(
                    "invalid node text `{trimmed}` — wrap node text in [brackets] or \"quotes\""
                ));
            }
            let (id, label, shape) = parse_node_token(trimmed);
            if !id.is_empty() {
                ensure_node(&id, &label, shape);
            }
        }
    }

    // Assemble the flat subgraph list into a nested tree (roots first)
    type SgFlat = (
        String,
        Option<String>,
        Option<usize>,
        Option<LayoutDirection>,
        Vec<String>,
    );
    fn build_subgraph_tree(
        flat: &[SgFlat],
        parent: Option<usize>,
        colored_sgs: &std::collections::HashMap<String, Color>,
    ) -> Vec<SubgraphSpec> {
        let mut out = Vec::new();
        for (i, (id, title, p, sg_dir, members)) in flat.iter().enumerate() {
            if *p == parent {
                out.push(SubgraphSpec {
                    id: id.clone(),
                    title: title.clone(),
                    color: colored_sgs.get(id).copied(),
                    direction: *sg_dir,
                    nodes: members.clone(),
                    subgraphs: build_subgraph_tree(flat, Some(i), colored_sgs),
                });
            }
        }
        out
    }
    let flat_tuples: Vec<SgFlat> = subgraphs_flat
        .into_inner()
        .into_iter()
        .map(|sg| (sg.id, sg.title, sg.parent, sg.direction, sg.members))
        .collect();
    let subgraphs = build_subgraph_tree(&flat_tuples, None, &colored_sgs);

    // Apply deferred style marks (class/style lines may precede node defs)
    for node in &mut nodes {
        if dashed_nodes.contains(&node.id) {
            node.dashed_border = true;
        }
        if let Some(c) = colored_nodes.get(&node.id) {
            node.color = Some(*c);
        }
        if let Some(c) = filled_nodes.get(&node.id) {
            node.fill_color = Some(*c);
        }
        if let Some(&lvl) = border_nodes.get(&node.id) {
            node.border_level = lvl;
        }
    }
    for (i, edge) in edges.iter_mut().enumerate() {
        if dashed_links.contains(&i) {
            edge.dashed = true;
        }
        if let Some(c) = colored_links.get(&i) {
            edge.color = Some(*c);
        }
    }

    Ok(DiagramSpec::Flowchart(FlowchartSpec {
        direction,
        style: default_style,
        title: None,
        nodes,
        edges,
        subgraphs,
    }))
}

/// Replaces statement-separating `;` with newlines, skipping `;` inside
/// brackets or quotes so labels like `[a; b]` survive untouched.
fn normalize_semicolons(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut depth = 0usize;
    let mut in_quote: Option<char> = None;
    for ch in input.chars() {
        match ch {
            q @ ('"' | '\'') if in_quote.is_none() => {
                in_quote = Some(q);
                out.push(q);
            }
            q if Some(q) == in_quote => {
                in_quote = None;
                out.push(q);
            }
            '[' | '{' | '(' if in_quote.is_none() => {
                depth += 1;
                out.push(ch);
            }
            ']' | '}' | ')' if in_quote.is_none() && depth > 0 => {
                depth -= 1;
                out.push(ch);
            }
            ';' if in_quote.is_none() && depth == 0 => out.push('\n'),
            _ => out.push(ch),
        }
    }
    out
}

fn clean_label(raw: &str) -> String {
    let mut s = raw.trim().to_string();
    if (s.starts_with('"') && s.ends_with('"')) || (s.starts_with('\'') && s.ends_with('\'')) {
        s = s[1..s.len().saturating_sub(1)].to_string();
    }
    s = s
        .replace("\\n", "\n")
        .replace("<br/>", "\n")
        .replace("<br>", "\n")
        .replace("<br />", "\n");
    s
}

/// Is `c` a character that only occurs inside shape wrappers or quotes?
fn is_node_token_special(c: char) -> bool {
    c.is_whitespace() || matches!(c, '[' | ']' | '{' | '}' | '(' | ')' | '"' | '\'')
}

/// Closed node-token grammar: does this token consist of exactly one
/// complete node spec with zero residue? Accepts (a) one shape wrapper using
/// the same patterns [`parse_node_token`] recognizes — `[(..)]`, `([..])`,
/// `[[..]]`, `/../` or `[..\\]` (see [`bracket_slash_open`]), `[..]`,
/// `(((..)))`, `((..))`, `(..)`, `{{..}}`, `{..}` — where the wrapper closes
/// at the very end of the token and only a clean node id precedes it (an
/// empty label is allowed, e.g. `B[]`), (b) a fully quoted string (outer
/// quotes stripped later by [`clean_label`]), or (c) a bare identifier with
/// no whitespace and no bracket/quote characters.
///
/// Every documented construct fully consumes its token — `|edge labels|` are
/// stripped before target extraction, whitespace is legal only inside
/// quotes/brackets, and bare node ids are single lexical tokens — so any
/// residue is necessarily a typo, unsupported arrow, or prose, and must
/// error instead of being silently absorbed into a node id.
fn node_token_fully_consumed(token: &str) -> bool {
    let t = token.trim();
    if t.is_empty() {
        return false;
    }

    // (b) fully quoted label (quotes may wrap whitespace)
    if t.len() >= 2
        && ((t.starts_with('"') && t.ends_with('"')) || (t.starts_with('\'') && t.ends_with('\'')))
    {
        return true;
    }

    // (a) shape wrappers, checked in parse_node_token's order. Each check
    // mirrors parse_node_token's find/rfind semantics and additionally
    // requires the closing needle to end the token and the id prefix before
    // the wrapper to be a clean bare identifier.
    let id_ok = |prefix: &str| -> bool {
        let p = prefix.trim();
        !p.is_empty() && !p.chars().any(is_node_token_special)
    };

    // [(Database)]
    if let Some(start) = t.find("[(")
        && let Some(end) = t.rfind(")]")
        && end + 2 == t.len()
        && id_ok(&t[..start])
    {
        return true;
    }
    // ([Stadium])
    if let Some(start) = t.find("([")
        && let Some(end) = t.rfind("])")
        && end + 2 == t.len()
        && id_ok(&t[..start])
    {
        return true;
    }
    // [[Subprocess]]
    if let Some(start) = t.find("[[")
        && let Some(end) = t.rfind("]]")
        && end + 2 == t.len()
        && id_ok(&t[..start])
    {
        return true;
    }
    // [/Parallelogram] | [/Trapezoid\] | [\ParallelogramAlt\] | [\TrapezoidAlt/]
    if let Some((start, _)) = bracket_slash_open(t)
        && let Some(end) = t.rfind(']')
        && end + 1 == t.len()
        && start + 2 < end
        && id_ok(&t[..start])
    {
        return true;
    }
    // [Box]
    if let Some(start) = t.find('[')
        && let Some(end) = t.rfind(']')
        && end + 1 == t.len()
        && id_ok(&t[..start])
    {
        return true;
    }
    // (((DoubleCircle)))
    if let Some(start) = t.find("(((")
        && let Some(end) = t.rfind(")))")
        && end + 3 == t.len()
        && id_ok(&t[..start])
    {
        return true;
    }
    // ((Circle))
    if let Some(start) = t.find("((")
        && let Some(end) = t.rfind("))")
        && end + 2 == t.len()
        && id_ok(&t[..start])
    {
        return true;
    }
    // (Rounded)
    if let Some(start) = t.find('(')
        && let Some(end) = t.rfind(')')
        && end + 1 == t.len()
        && id_ok(&t[..start])
    {
        return true;
    }
    // {{Hexagon}}
    if let Some(start) = t.find("{{")
        && let Some(end) = t.rfind("}}")
        && end + 2 == t.len()
        && id_ok(&t[..start])
    {
        return true;
    }
    // {Diamond}
    if let Some(start) = t.find('{')
        && let Some(end) = t.rfind('}')
        && end + 1 == t.len()
        && id_ok(&t[..start])
    {
        return true;
    }

    // (c) bare identifier: no whitespace, no bracket/quote characters
    !t.chars().any(is_node_token_special)
}

fn parse_node_token(token: &str) -> (String, String, NodeShape) {
    let t = token.trim();

    // [(Database)]
    if let Some(start) = t.find("[(")
        && let Some(end) = t.rfind(")]")
    {
        let id = t[..start].trim().to_string();
        let label = clean_label(&t[start + 2..end]);
        return (id, label, NodeShape::Database);
    }

    // ([Stadium])
    if let Some(start) = t.find("([")
        && let Some(end) = t.rfind("])")
    {
        let id = t[..start].trim().to_string();
        let label = clean_label(&t[start + 2..end]);
        return (id, label, NodeShape::Stadium);
    }

    // [[Subprocess]]
    if let Some(start) = t.find("[[")
        && let Some(end) = t.rfind("]]")
    {
        let id = t[..start].trim().to_string();
        let label = clean_label(&t[start + 2..end]);
        return (id, label, NodeShape::Subprocess);
    }

    // [/Parallelogram] | [/Trapezoid\] | [\ParallelogramAlt\] | [\TrapezoidAlt/]
    if let Some((start, open)) = bracket_slash_open(t)
        && let Some(end) = t.rfind(']')
        && start + 2 < end
    {
        let close = t.as_bytes()[end - 1] as char;
        let label = clean_label(&t[start + 2..end - 1]);
        let shape = match (open, close) {
            ('/', '/') => NodeShape::Parallelogram,
            ('\\', '\\') => NodeShape::ParallelogramAlt,
            ('/', '\\') => NodeShape::Trapezoid,
            (_, '/') => NodeShape::TrapezoidAlt,
            _ => NodeShape::Box,
        };
        let id = t[..start].trim().to_string();
        return (id, label, shape);
    }

    // [Box]
    if let Some(start) = t.find('[')
        && let Some(end) = t.rfind(']')
    {
        let id = t[..start].trim().to_string();
        let label = clean_label(&t[start + 1..end]);
        return (id, label, NodeShape::Box);
    }

    // (((DoubleCircle)))
    if let Some(start) = t.find("(((")
        && let Some(end) = t.rfind(")))")
    {
        let id = t[..start].trim().to_string();
        let label = clean_label(&t[start + 3..end]);
        return (id, label, NodeShape::DoubleCircle);
    }

    // ((Circle))
    if let Some(start) = t.find("((")
        && let Some(end) = t.rfind("))")
    {
        let id = t[..start].trim().to_string();
        let label = clean_label(&t[start + 2..end]);
        return (id, label, NodeShape::Circle);
    }

    // (Rounded)
    if let Some(start) = t.find('(')
        && let Some(end) = t.rfind(')')
    {
        let id = t[..start].trim().to_string();
        let label = clean_label(&t[start + 1..end]);
        return (id, label, NodeShape::Rounded);
    }

    // {{Hexagon}}
    if let Some(start) = t.find("{{")
        && let Some(end) = t.rfind("}}")
    {
        let id = t[..start].trim().to_string();
        let label = clean_label(&t[start + 2..end]);
        return (id, label, NodeShape::Hexagon);
    }

    // {Diamond}
    if let Some(start) = t.find('{')
        && let Some(end) = t.rfind('}')
    {
        let id = t[..start].trim().to_string();
        let label = clean_label(&t[start + 1..end]);
        return (id, label, NodeShape::Diamond);
    }

    let cleaned = clean_label(t);
    (cleaned.clone(), cleaned, NodeShape::Box)
}

/// Finds `[/` or `[\` (bracket followed by a slash) for parallelogram /
/// trapezoid tokens. Returns the index of `[` and the slash character.
fn bracket_slash_open(t: &str) -> Option<(usize, char)> {
    let bytes = t.as_bytes();
    for i in 0..bytes.len().saturating_sub(1) {
        if bytes[i] == b'[' && (bytes[i + 1] == b'/' || bytes[i + 1] == b'\\') {
            return Some((i, bytes[i + 1] as char));
        }
    }
    None
}

fn find_next_delim(text: &str) -> Option<(usize, &'static str)> {
    // Longer variants must precede their prefixes: the earliest-position
    // tie-break below keeps the first table entry on equal index (strict
    // `<`), so `<-->` wins over `<--` and `<==>` over `<==` at the same
    // occurrence. `<-.->` starts one char earlier than `-.->` at the same
    // spot, but listing it first documents intent.
    let delimiters = [
        "<-.->", "<==>", "<-->", "==>", "<--", "-.->", "-->", "<==", "---",
    ];
    let mut earliest: Option<(usize, &'static str)> = None;

    for delim in delimiters {
        if let Some(idx) = text.find(delim) {
            match earliest {
                Some((min_idx, _)) if idx < min_idx => {
                    earliest = Some((idx, delim));
                }
                None => {
                    earliest = Some((idx, delim));
                }
                _ => {}
            }
        }
    }
    earliest
}

fn split_bracket_aware(s: &str, delimiter: char) -> Vec<&str> {
    let mut result = Vec::new();
    let mut depth = 0;
    let mut last_idx = 0;

    for (idx, ch) in s.char_indices() {
        match ch {
            '[' | '{' | '(' => depth += 1,
            ']' | '}' | ')' => {
                if depth > 0 {
                    depth -= 1;
                }
            }
            c if c == delimiter && depth == 0 => {
                let part = s[last_idx..idx].trim();
                if !part.is_empty() {
                    result.push(part);
                }
                last_idx = idx + c.len_utf8();
            }
            _ => {}
        }
    }

    let remainder = s[last_idx..].trim();
    if !remainder.is_empty() {
        result.push(remainder);
    }

    result
}

fn parse_flowchart_edges_in_line<F>(
    mut line: &str,
    ensure_node: &mut F,
    edges: &mut Vec<EdgeSpec>,
) -> Result<(), String>
where
    F: FnMut(&str, &str, NodeShape),
{
    while let Some((idx, delim)) = find_next_delim(line) {
        let left_part = line[..idx].trim();
        let mut rest = line[idx + delim.len()..].trim();

        let mut edge_label = None;
        if rest.starts_with('|')
            && let Some(end_bar) = rest[1..].find('|')
        {
            edge_label = Some(clean_label(&rest[1..=end_bar]));
            rest = rest[end_bar + 2..].trim();
        }

        let (target_part, has_next) = match find_next_delim(rest) {
            Some((next_idx, _)) => (rest[..next_idx].trim(), true),
            None => (rest, false),
        };

        let arrow = match delim {
            "---" => ArrowDirection::None,
            "<-->" | "<==>" | "<-.->" => ArrowDirection::Both,
            "<--" | "<==" => ArrowDirection::Back,
            _ => ArrowDirection::Forward,
        };
        let is_dashed = delim == "-.->" || delim == "<-.->";
        let is_thick = delim == "==>" || delim == "<==>" || delim == "<==";

        let sources = split_bracket_aware(left_part, '&');
        let targets = split_bracket_aware(target_part, '&');

        if sources.is_empty() {
            return Err(format!(
                "invalid node text `{left_part}` in edge statement — wrap node text in [brackets] or \"quotes\""
            ));
        }
        if targets.is_empty() {
            return Err(format!(
                "invalid node text `{target_part}` in edge statement — wrap node text in [brackets] or \"quotes\""
            ));
        }

        // Reject tokens with residue: anything outside the closed node-token
        // grammar is a typo, unsupported arrow, or prose, never a documented
        // construct (|labels| are stripped above, whitespace is legal only
        // inside quotes/brackets)
        for token in sources.iter().chain(targets.iter()) {
            if !node_token_fully_consumed(token) {
                return Err(format!(
                    "invalid node text `{token}` in edge statement — wrap node text in [brackets] or \"quotes\""
                ));
            }
        }

        for s in &sources {
            let (u_id, u_label, u_shape) = parse_node_token(s);
            ensure_node(&u_id, &u_label, u_shape);

            for t in &targets {
                let (v_id, v_label, v_shape) = parse_node_token(t);
                ensure_node(&v_id, &v_label, v_shape);

                edges.push(EdgeSpec {
                    from: u_id.clone(),
                    to: v_id,
                    label: edge_label.clone(),
                    arrow,
                    dashed: is_dashed,
                    thick: is_thick,
                    color: None,
                });
            }
        }

        if !has_next {
            break;
        }
        line = rest;
    }
    Ok(())
}

/// Parses a Mermaid sequence-diagram DSL into a [`SequenceSpec`].
///
/// # Errors
///
/// Returns `Err` only when no messages could be parsed and at least one line
/// was unrecognized (with 1-based line numbers); lenient otherwise, matching
/// the parser's best-effort posture for machine-generated DSL.
pub fn parse_sequence_dsl(input: &str, default_style: BoxStyle) -> Result<DiagramSpec, String> {
    let mut participants = Vec::new();
    let mut p_set = std::collections::HashSet::new();
    let mut messages = Vec::new();
    let mut frames = Vec::new();
    let mut notes: Vec<SeqNoteSpec> = Vec::new();
    // Stack of in-progress frames; `end` pops and commits them (supports nesting)
    let mut open_frames: Vec<SeqFrameSpec> = Vec::new();
    // Unrecognized (1-based line number, trimmed text) for diagnostics
    let mut unrecognized: Vec<(usize, String)> = Vec::new();

    let add_participant = |id: &str,
                           label: Option<String>,
                           p_list: &mut Vec<ParticipantSpec>,
                           p_s: &mut std::collections::HashSet<String>| {
        if !p_s.contains(id) {
            p_s.insert(id.to_string());
            p_list.push(ParticipantSpec {
                id: id.to_string(),
                label,
                color: None,
            });
        }
    };

    for (ln, line) in input.lines().enumerate() {
        // `sequenceDiagram` header; a same-line statement suffix (Mermaid
        // `sequenceDiagram A->>B: hi`) is parsed as the first statement.
        // `parse_dsl_or_json` normalizes `;` to newlines, so direct pub calls
        // with multi-statement `;` suffixes degrade to one statement + an
        // unrecognized-line diagnostic.
        let mut trimmed = line.trim();
        if trimmed.starts_with("sequenceDiagram") {
            let after = &trimmed["sequenceDiagram".len()..];
            if after.is_empty()
                || after.starts_with(';')
                || after.starts_with(|c: char| c.is_whitespace())
            {
                let rest = after.trim_start_matches(';').trim();
                if rest.is_empty() {
                    continue;
                }
                trimmed = rest;
            }
        }
        if trimmed.is_empty() || trimmed.starts_with("%%") {
            continue;
        }

        // participant A [as Label]
        if let Some(rest) = trimmed
            .strip_prefix("participant ")
            .or_else(|| trimmed.strip_prefix("actor "))
        {
            if let Some(as_pos) = rest.find(" as ") {
                let id = rest[..as_pos].trim();
                let label = rest[as_pos + 4..].trim().trim_matches('"');
                let unescaped = label
                    .replace("\\n", "\n")
                    .replace("<br/>", "\n")
                    .replace("<br>", "\n")
                    .replace("<br />", "\n");
                add_participant(id, Some(unescaped), &mut participants, &mut p_set);
            } else {
                let id = rest.trim();
                add_participant(id, None, &mut participants, &mut p_set);
            }
            continue;
        }

        // Notes: `note over A[,B]: text`, `note right of X: text`,
        // `note left of X: text` (keyword case-insensitive, Mermaid form).
        // Notes introduce participants, matching Mermaid. Malformed note
        // forms fall through to the unrecognized-line tracking below.
        if trimmed.len() >= 5
            && trimmed[..4].eq_ignore_ascii_case("note")
            && (trimmed.as_bytes()[4] == b' ' || trimmed.as_bytes()[4] == b'\t')
        {
            let rest = trimmed[4..].trim();
            let (targets_raw, text_raw) = match rest.find(':') {
                Some(c) => (&rest[..c], rest[c + 1..].trim()),
                None => (rest, ""),
            };
            let targets_lc = targets_raw.to_ascii_lowercase();
            let ids: Vec<String> = if targets_lc == "over" || targets_lc.starts_with("over ") {
                targets_raw[4..]
                    .split(',')
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(String::from)
                    .collect()
            } else if targets_lc == "right of" || targets_lc.starts_with("right of ") {
                let id = targets_raw[8..].trim();
                if id.is_empty() {
                    Vec::new()
                } else {
                    vec![id.to_string()]
                }
            } else if targets_lc == "left of" || targets_lc.starts_with("left of ") {
                let id = targets_raw[7..].trim();
                if id.is_empty() {
                    Vec::new()
                } else {
                    vec![id.to_string()]
                }
            } else {
                Vec::new()
            };
            if !ids.is_empty() {
                let text = text_raw
                    .trim_matches('"')
                    .replace("\\n", " ")
                    .replace("<br/>", " ")
                    .replace("<br>", " ")
                    .replace("<br />", " ");
                for id in &ids {
                    add_participant(id, None, &mut participants, &mut p_set);
                }
                notes.push(SeqNoteSpec {
                    over: ids,
                    text,
                    at_step: messages.len(),
                    position: if targets_lc.starts_with("right of") {
                        SeqNotePosition::RightOf
                    } else if targets_lc.starts_with("left of") {
                        SeqNotePosition::LeftOf
                    } else {
                        SeqNotePosition::Over
                    },
                });
                continue;
            }
        }

        // Control-flow frames: alt/opt/loop/par/critical/break + else/and branches + end
        let frame_keywords = ["alt", "opt", "loop", "par", "critical", "break"];
        if let Some(kw) = frame_keywords.iter().find(|kw| {
            trimmed
                .strip_prefix(**kw)
                .is_some_and(|r| r.is_empty() || r.starts_with(' '))
        }) {
            let cond = trimmed[kw.len()..].trim();
            open_frames.push(SeqFrameSpec {
                label: (*kw).to_string(),
                branches: vec![cond.to_string()],
                branch_steps: vec![messages.len()],
                start_step: messages.len(),
                end_step: messages.len(),
            });
            continue;
        }
        if trimmed == "end" || trimmed == "end(" || trimmed.starts_with("end ") {
            if let Some(mut frame) = open_frames.pop() {
                frame.end_step = messages.len();
                frames.push(frame);
            }
            continue;
        }
        // Branch splits: `else` (alt/opt), `and` (par), `option` (critical only)
        let opt_rest = if open_frames.last().is_some_and(|f| f.label == "critical") {
            trimmed.strip_prefix("option ")
        } else {
            None
        };
        if let Some(rest) = trimmed
            .strip_prefix("else ")
            .or_else(|| trimmed.strip_prefix("and "))
            .or(opt_rest)
        {
            if let Some(frame) = open_frames.last_mut() {
                frame.branches.push(rest.trim().to_string());
                frame.branch_steps.push(messages.len());
            }
            continue;
        }

        // Messages: A ->> B: Msg or A -> B: Msg or A --> B: Msg
        let arrow_patterns = ["-->>", "-->", "<->", "->>", "->"];
        let mut matched = false;
        for pat in arrow_patterns {
            if let Some(idx) = trimmed.find(pat) {
                let from_id = trimmed[..idx].trim();
                let rest = trimmed[idx + pat.len()..].trim();

                let (to_id, label) = if let Some(colon) = rest.find(':') {
                    (&rest[..colon].trim(), rest[colon + 1..].trim())
                } else {
                    (&rest, "")
                };

                let clean_label = label
                    .trim_matches('"')
                    .replace("\\n", " ")
                    .replace("<br/>", " ")
                    .replace("<br>", " ")
                    .replace("<br />", " ");

                add_participant(from_id, None, &mut participants, &mut p_set);
                add_participant(to_id, None, &mut participants, &mut p_set);

                let m_type = match pat {
                    "-->" | "-->>" => SeqMessageType::Async,
                    "<->" => SeqMessageType::Bidirectional,
                    _ => SeqMessageType::Sync,
                };

                messages.push(SeqMessageSpec {
                    from: from_id.to_string(),
                    to: to_id.to_string(),
                    label: clean_label,
                    message_type: m_type,
                });
                matched = true;
                break;
            }
        }
        if !matched {
            unrecognized.push((ln + 1, trimmed.to_string()));
        }
    }

    // Lenient auto-commit: frames left open at EOF close after the last
    // message. Pop order (innermost first) matches `end`-commit order, so
    // frames-vec nesting order (inner = lower index) is preserved.
    while let Some(mut frame) = open_frames.pop() {
        frame.end_step = messages.len();
        frames.push(frame);
    }

    // Diagnostics: junk is tolerated once something parsed, but fully
    // unrecognized input is reported with line numbers.
    if messages.is_empty() && !unrecognized.is_empty() {
        return Err(format!(
            "sequenceDiagram: no messages parsed; unrecognized line(s): {}",
            unrecognized
                .iter()
                .map(|(n, l)| format!("line {n}: `{l}`"))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }

    Ok(DiagramSpec::Sequence(SequenceSpec {
        style: default_style,
        title: None,
        participants,
        messages,
        notes,
        frames,
    }))
}

/// Parses an indented tree DSL into a [`TreeSpec`].
///
/// # Errors
///
/// Returns `Err` if the input contains no tree lines.
///
/// # Panics
///
/// Not expected in practice: the root sentinel keeps the indent stack
/// non-empty, so every `unwrap()` is guarded by a `stack.len() > 1` check.
/// Splits a trailing `@<name|#hex>` color tag off `s`. The tag must parse
/// as a color name or `#hex`, so labels containing literal `@` (`user@host`)
/// pass through untouched. Returns the trimmed rest and the parsed color.
fn split_trailing_color(s: &str) -> (&str, Option<Color>) {
    if let Some(idx) = s.rfind('@')
        && let Some(c) = Color::parse(s[idx + 1..].trim())
    {
        return (s[..idx].trim_end(), Some(c));
    }
    (s, None)
}

pub fn parse_tree_dsl(input: &str, default_style: BoxStyle) -> Result<DiagramSpec, String> {
    let mut lines = input.lines().filter(|l| !l.trim().is_empty());
    let Some(first) = lines.next() else {
        return Err("Empty tree specification".to_string());
    };

    let root_name = if first.trim() == "tree" {
        lines.next().unwrap_or("Root").trim()
    } else {
        first.trim()
    };

    let clean_root = root_name.trim_start_matches("- ").trim();
    let root = TreeNodeSpec {
        name: clean_root.to_string(),
        annotation: None,
        children: Vec::new(),
        color: None,
    };

    // Stack of (indent_level, node)
    let mut stack: Vec<(usize, TreeNodeSpec)> = vec![(0, root)];

    for line in lines {
        let trimmed_line = line.trim();
        if trimmed_line.is_empty() {
            continue;
        }

        // Count leading spaces
        let indent = line.chars().take_while(|c| *c == ' ' || *c == '\t').count();
        let content = trimmed_line.trim_start_matches("- ").trim();

        // Trailing `@<color>` tag applies before annotation extraction so
        // `API (port 8080) @blue` keeps both annotation and color
        let (content, node_color) = split_trailing_color(content);
        let (name, ann) = if let Some(start) = content.find('(') {
            if let Some(end) = content.rfind(')') {
                let n = content[..start].trim().to_string();
                let a = content[start + 1..end].trim().to_string();
                (n, Some(a))
            } else {
                (content.to_string(), None)
            }
        } else {
            (content.to_string(), None)
        };

        let node = TreeNodeSpec {
            name,
            annotation: ann,
            children: Vec::new(),
            color: node_color,
        };

        while stack.len() > 1 && stack.last().unwrap().0 >= indent {
            let (_, child) = stack.pop().unwrap();
            stack.last_mut().unwrap().1.children.push(child);
        }

        stack.push((indent, node));
    }

    while stack.len() > 1 {
        let (_, child) = stack.pop().unwrap();
        stack.last_mut().unwrap().1.children.push(child);
    }

    Ok(DiagramSpec::Tree(TreeSpec {
        style: default_style,
        root: stack.pop().unwrap().1,
    }))
}

/// Parses a memory/stack DSL into a [`StackSpec`].
///
/// # Errors
///
/// Currently always returns `Ok`; the `Result` keeps the parser signatures
/// uniform with the other DSL parsers.
pub fn parse_stack_dsl(input: &str, default_style: BoxStyle) -> Result<DiagramSpec, String> {
    let mut layers = Vec::new();
    let mut title = None;
    let mut bottom_address = None;

    for (idx, line) in input.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        if idx == 0
            && (trimmed.starts_with("stack")
                || trimmed.starts_with("memory-map")
                || trimmed.starts_with("memory"))
        {
            let rest = trimmed
                .strip_prefix("memory-map")
                .or_else(|| trimmed.strip_prefix("memory"))
                .or_else(|| trimmed.strip_prefix("stack"))
                .unwrap_or(trimmed)
                .trim();
            if !rest.is_empty() {
                title = Some(rest.to_string());
            }
            continue;
        }

        if trimmed.to_lowercase().starts_with("title:") {
            title = Some(trimmed["title:".len()..].trim().to_string());
            continue;
        }

        // 0xFFFF: Label (description) or 0x0000: (bottom address)
        let (addr, rest) = if let Some(col) = trimmed.find(':') {
            (
                Some(trimmed[..col].trim().to_string()),
                trimmed[col + 1..].trim(),
            )
        } else {
            (None, trimmed)
        };

        let clean_rest = rest.trim_start_matches("- ").trim();
        if clean_rest.is_empty() && addr.is_some() {
            bottom_address = addr;
            continue;
        }

        // Trailing `@<color>` tag (before annotation extraction)
        let (clean_rest, layer_color) = split_trailing_color(clean_rest);
        let (label, desc) = if let Some(start) = clean_rest.find('(') {
            if let Some(end) = clean_rest.rfind(')') {
                let l = clean_rest[..start].trim().to_string();
                let d = clean_rest[start + 1..end].trim().to_string();
                (l, Some(d))
            } else {
                (clean_rest.to_string(), None)
            }
        } else {
            (clean_rest.to_string(), None)
        };

        layers.push(StackLayerSpec {
            label,
            address_or_id: addr,
            description: desc,
            color: layer_color,
        });
    }

    Ok(DiagramSpec::Stack(StackSpec {
        style: default_style,
        title,
        layers,
        bottom_address,
        grows_down: true,
    }))
}

/// Parses a pipe-delimited table DSL into a [`TableSpec`].
///
/// # Errors
///
/// Returns `Err` if no valid table rows are found.
pub fn parse_table_dsl(input: &str, default_style: BoxStyle) -> Result<DiagramSpec, String> {
    let mut headers = Vec::new();
    let mut rows = Vec::new();
    let mut alignments = Vec::new();
    let mut table_color = None;

    for line in input.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("table") {
            continue;
        }

        // `color: <name|#hex>` — grid/border color for the whole table
        if let Some(rest) = trimmed.strip_prefix("color:") {
            if let Some(c) = Color::parse(rest.trim()) {
                table_color = Some(c);
            }
            continue;
        }

        if !trimmed.contains('|') {
            continue;
        }

        let mut cells: Vec<String> = trimmed.split('|').map(|c| c.trim().to_string()).collect();

        // Drop empty first/last if leading/trailing '|'
        if cells.first().is_some_and(std::string::String::is_empty) {
            cells.remove(0);
        }
        if cells.last().is_some_and(std::string::String::is_empty) {
            cells.pop();
        }
        if cells.is_empty() {
            continue;
        }

        // Check if separator line (e.g. |---|:---|---:|)
        let is_sep = cells.iter().all(|c| {
            let inner = c.trim();
            !inner.is_empty() && inner.chars().all(|ch| ch == '-' || ch == ':' || ch == ' ')
        });

        if is_sep {
            alignments = cells
                .iter()
                .map(|c| {
                    let left = c.starts_with(':');
                    let right = c.ends_with(':');
                    if left && right {
                        TextAlign::Center
                    } else if right {
                        TextAlign::Right
                    } else {
                        TextAlign::Left
                    }
                })
                .collect();
            continue;
        }

        if headers.is_empty() {
            headers = cells;
        } else {
            rows.push(cells);
        }
    }

    if headers.is_empty() {
        return Err("No valid table rows found".to_string());
    }

    Ok(DiagramSpec::Table(TableSpec {
        style: default_style,
        color: table_color,
        headers,
        rows,
        alignments,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_mermaid_flowchart() {
        let dsl = r"
        graph TD
          Client[Web Client] -->|GET /users| Gateway[API Gateway]
          Gateway --> ServiceA[User Service]
          Gateway --> ServiceB[Auth Service]
        ";

        let spec = parse_dsl_or_json(dsl, BoxStyle::Rounded).unwrap();
        match spec {
            DiagramSpec::Flowchart(f) => {
                assert_eq!(f.nodes.len(), 4);
                assert_eq!(f.edges.len(), 3);
                assert_eq!(f.edges[0].label, Some("GET /users".to_string()));
            }
            _ => panic!("Expected flowchart"),
        }
    }

    #[test]
    fn test_parse_new_node_shapes() {
        let dsl = r"
        graph TB
          H{{Hexagon}} --> D(((Double)))
          D --> P[/Parallelogram/]
          P --> PA[\ParallelogramAlt\]
          PA --> T[/Trapezoid\]
          T --> TA[\TrapezoidAlt/]
        ";
        let spec = parse_dsl_or_json(dsl, BoxStyle::Rounded).unwrap();
        match spec {
            DiagramSpec::Flowchart(f) => {
                let shape_of = |id: &str| {
                    f.nodes
                        .iter()
                        .find(|n| n.id == id)
                        .map(|n| n.shape)
                        .unwrap_or(NodeShape::Box)
                };
                assert_eq!(shape_of("H"), NodeShape::Hexagon);
                assert_eq!(shape_of("D"), NodeShape::DoubleCircle);
                assert_eq!(shape_of("P"), NodeShape::Parallelogram);
                assert_eq!(shape_of("PA"), NodeShape::ParallelogramAlt);
                assert_eq!(shape_of("T"), NodeShape::Trapezoid);
                assert_eq!(shape_of("TA"), NodeShape::TrapezoidAlt);
                assert_eq!(f.edges.len(), 5);
            }
            _ => panic!("Expected flowchart"),
        }
    }

    #[test]
    fn test_parse_existing_shapes_unchanged() {
        let dsl =
            "graph TB; A[Box]; B(Rounded); C{Diamond}; D[(Db)]; E[[Sub]]; F([Stad]); G((Circle))";
        let spec = parse_dsl_or_json(dsl, BoxStyle::Rounded).unwrap();
        match spec {
            DiagramSpec::Flowchart(f) => {
                let shape_of = |id: &str| {
                    f.nodes
                        .iter()
                        .find(|n| n.id == id)
                        .map(|n| n.shape)
                        .unwrap_or(NodeShape::Box)
                };
                assert_eq!(shape_of("A"), NodeShape::Box);
                assert_eq!(shape_of("B"), NodeShape::Rounded);
                assert_eq!(shape_of("C"), NodeShape::Diamond);
                assert_eq!(shape_of("D"), NodeShape::Database);
                assert_eq!(shape_of("E"), NodeShape::Subprocess);
                assert_eq!(shape_of("F"), NodeShape::Stadium);
                assert_eq!(shape_of("G"), NodeShape::Circle);
            }
            _ => panic!("Expected flowchart"),
        }
    }

    #[test]
    fn test_parse_thick_edges() {
        let dsl = "graph TB; A ==> B; B <==> C";
        let spec = parse_dsl_or_json(dsl, BoxStyle::Rounded).unwrap();
        match spec {
            DiagramSpec::Flowchart(f) => {
                assert_eq!(f.edges.len(), 2);
                assert!(f.edges[0].thick, "==> must set thick");
                assert_eq!(f.edges[0].arrow, ArrowDirection::Forward);
                assert!(f.edges[1].thick, "<==> must set thick");
                assert_eq!(f.edges[1].arrow, ArrowDirection::Both);
                assert!(!f.edges[0].dashed && !f.edges[1].dashed);
            }
            _ => panic!("Expected flowchart"),
        }
    }

    #[test]
    fn test_parse_bidirectional_dashed_and_reverse_thick_edges() {
        // (1) 'A <-.-> B' -> 1 edge Both+dashed, nodes exactly {A,B}
        let dsl = "graph TD; A <-.-> B";
        let spec = parse_dsl_or_json(dsl, BoxStyle::Rounded).unwrap();
        match spec {
            DiagramSpec::Flowchart(f) => {
                assert_eq!(f.edges.len(), 1);
                assert_eq!(f.edges[0].from, "A");
                assert_eq!(f.edges[0].to, "B");
                assert_eq!(f.edges[0].arrow, ArrowDirection::Both);
                assert!(f.edges[0].dashed);
                assert!(!f.edges[0].thick);
                assert_eq!(f.nodes.len(), 2);
            }
            _ => panic!("Expected flowchart"),
        }

        // (2) 'A <== B' -> Back+thick
        let dsl2 = "graph TD; A <== B";
        let spec2 = parse_dsl_or_json(dsl2, BoxStyle::Rounded).unwrap();
        match spec2 {
            DiagramSpec::Flowchart(f) => {
                assert_eq!(f.edges.len(), 1);
                assert_eq!(f.edges[0].from, "A");
                assert_eq!(f.edges[0].to, "B");
                assert_eq!(f.edges[0].arrow, ArrowDirection::Back);
                assert!(f.edges[0].thick);
                assert!(!f.edges[0].dashed);
            }
            _ => panic!("Expected flowchart"),
        }

        // (3) regressions '<==>','-.->','<-->','<--','---' unchanged
        let dsl3 = "graph TD; A <==> B; B -.-> C; C <--> D; D <-- E; E --- F";
        let spec3 = parse_dsl_or_json(dsl3, BoxStyle::Rounded).unwrap();
        match spec3 {
            DiagramSpec::Flowchart(f) => {
                assert_eq!(f.edges.len(), 5);
                assert_eq!(f.edges[0].arrow, ArrowDirection::Both);
                assert!(f.edges[0].thick);
                assert_eq!(f.edges[1].arrow, ArrowDirection::Forward);
                assert!(f.edges[1].dashed);
                assert_eq!(f.edges[2].arrow, ArrowDirection::Both);
                assert!(!f.edges[2].dashed && !f.edges[2].thick);
                assert_eq!(f.edges[3].arrow, ArrowDirection::Back);
                assert!(!f.edges[3].dashed && !f.edges[3].thick);
                assert_eq!(f.edges[4].arrow, ArrowDirection::None);
                assert!(!f.edges[4].dashed && !f.edges[4].thick);
            }
            _ => panic!("Expected flowchart"),
        }

        // (4) chained 'A <-.-> B <== C' -> 2 edges correct flags
        let dsl4 = "graph TD; A <-.-> B <== C";
        let spec4 = parse_dsl_or_json(dsl4, BoxStyle::Rounded).unwrap();
        match spec4 {
            DiagramSpec::Flowchart(f) => {
                assert_eq!(f.edges.len(), 2);
                assert_eq!(f.edges[0].from, "A");
                assert_eq!(f.edges[0].to, "B");
                assert_eq!(f.edges[0].arrow, ArrowDirection::Both);
                assert!(f.edges[0].dashed);
                assert!(!f.edges[0].thick);

                assert_eq!(f.edges[1].from, "B");
                assert_eq!(f.edges[1].to, "C");
                assert_eq!(f.edges[1].arrow, ArrowDirection::Back);
                assert!(f.edges[1].thick);
                assert!(!f.edges[1].dashed);
            }
            _ => panic!("Expected flowchart"),
        }

        // (5) 'A <-.->|lbl| B' parses
        let dsl5 = "graph TD; A <-.->|lbl| B";
        let spec5 = parse_dsl_or_json(dsl5, BoxStyle::Rounded).unwrap();
        match spec5 {
            DiagramSpec::Flowchart(f) => {
                assert_eq!(f.edges.len(), 1);
                assert_eq!(f.edges[0].from, "A");
                assert_eq!(f.edges[0].to, "B");
                assert_eq!(f.edges[0].label.as_deref(), Some("lbl"));
                assert_eq!(f.edges[0].arrow, ArrowDirection::Both);
                assert!(f.edges[0].dashed);
            }
            _ => panic!("Expected flowchart"),
        }
    }

    #[test]
    fn test_flowchart_token_validation_errors() {
        // (1) 'A --> B this is garbage' -> Err containing 'garbage'
        let err1 =
            parse_dsl_or_json("graph TD; A --> B this is garbage", BoxStyle::Rounded).unwrap_err();
        assert!(err1.contains("garbage"), "expected 'garbage' in: {err1}");

        // (2) 'A ~>> B' -> Err
        let err2 = parse_dsl_or_json("graph TD; A ~>> B", BoxStyle::Rounded).unwrap_err();
        assert!(
            err2.contains("A ~>> B"),
            "expected offending text in: {err2}"
        );

        // (3) 'garbage here --> B' -> Err
        let err3 =
            parse_dsl_or_json("graph TD; garbage here --> B", BoxStyle::Rounded).unwrap_err();
        assert!(
            err3.contains("garbage here"),
            "expected 'garbage here' in: {err3}"
        );

        // empty target edge 'A --> |lbl|' or 'A -->'
        assert!(parse_dsl_or_json("graph TD; A --> |lbl|", BoxStyle::Rounded).is_err());
        assert!(parse_dsl_or_json("graph TD; A -->", BoxStyle::Rounded).is_err());
        assert!(parse_dsl_or_json("graph TD; --> B", BoxStyle::Rounded).is_err());

        // prose chaining 'A --> B and B --> C' -> Err
        let err4 =
            parse_dsl_or_json("graph TD; A --> B and B --> C", BoxStyle::Rounded).unwrap_err();
        assert!(err4.contains("B and B"), "expected 'B and B' in: {err4}");
    }

    #[test]
    fn test_flowchart_token_validation_accepts_valid() {
        // (4) accept: 'A --> B[label with spaces]', quoted labels, '|edge label|', '& chains', 'A --> B --> C', labels containing ';'
        assert!(
            parse_dsl_or_json("graph TD; A --> B[label with spaces]", BoxStyle::Rounded).is_ok()
        );
        assert!(parse_dsl_or_json("graph TD; A --> \"quoted label\"", BoxStyle::Rounded).is_ok());
        assert!(parse_dsl_or_json("graph TD; \"quoted source\" --> B", BoxStyle::Rounded).is_ok());
        assert!(parse_dsl_or_json("graph TD; A -->|edge label| B", BoxStyle::Rounded).is_ok());
        assert!(parse_dsl_or_json("graph TD; A & B --> C & D", BoxStyle::Rounded).is_ok());
        assert!(parse_dsl_or_json("graph TD; A --> B --> C", BoxStyle::Rounded).is_ok());
        assert!(parse_dsl_or_json("graph TD; A[Foo; Bar] --> B", BoxStyle::Rounded).is_ok());
    }

    #[test]
    fn test_parse_semicolon_separators() {
        // Single-line CLI-style input: statements split on `;`, direction
        // suffix must not keep its trailing `;`
        let dsl = "graph LR; A[Start] --> B[End]";
        let spec = parse_dsl_or_json(dsl, BoxStyle::Rounded).unwrap();
        match spec {
            DiagramSpec::Flowchart(f) => {
                assert_eq!(f.direction, LayoutDirection::LR);
                assert_eq!(f.edges.len(), 1);
                assert_eq!(f.nodes.len(), 2);
            }
            _ => panic!("Expected flowchart"),
        }
    }

    #[test]
    fn test_semicolon_inside_label_preserved() {
        let dsl = "graph TD; A[Foo; Bar] --> B";
        let spec = parse_dsl_or_json(dsl, BoxStyle::Rounded).unwrap();
        match spec {
            DiagramSpec::Flowchart(f) => {
                let a = f.nodes.iter().find(|n| n.id == "A").unwrap();
                assert_eq!(a.label, "Foo; Bar");
            }
            _ => panic!("Expected flowchart"),
        }
    }

    #[test]
    fn test_parse_subgraph_style_color() {
        let dsl = "graph TB
            subgraph prod [Production]
                A --> B
            end
            C --> A
            style prod stroke:red";
        let spec = parse_dsl_or_json(dsl, BoxStyle::Rounded).unwrap();
        match spec {
            DiagramSpec::Flowchart(f) => {
                assert_eq!(f.subgraphs.len(), 1);
                assert_eq!(f.subgraphs[0].color, Some(Color::Red), "subgraph colored");
                assert_eq!(f.subgraphs[0].nodes.len(), 2, "members collected");
            }
            _ => panic!("Expected flowchart"),
        }
    }

    #[test]
    fn test_parse_class_and_style_directives() {
        let dsl = "graph TB
            A[Entry] --> B[Core]
            classDef ghost stroke-dasharray: 5 5,fill:#eee
            classDef solid fill:#f9f
            class A ghost
            style B stroke-dasharray: 4";
        let spec = parse_dsl_or_json(dsl, BoxStyle::Rounded).unwrap();
        match spec {
            DiagramSpec::Flowchart(f) => {
                assert_eq!(f.nodes.len(), 2);
                let a = f.nodes.iter().find(|n| n.id == "A").unwrap();
                let b = f.nodes.iter().find(|n| n.id == "B").unwrap();
                assert!(a.dashed_border, "class with dasharray");
                assert!(b.dashed_border, "style with dasharray");
                assert_eq!(a.color, None, "fill-only classDef leaves no color");
            }
            _ => panic!("Expected flowchart"),
        }
    }

    #[test]
    fn test_parse_classdef_stroke_color() {
        let dsl = "graph TB
            A[Entry] --> B[Core] --> C[Edge]
            classDef hot stroke:red
            classDef warn stroke:#ff8800,fill:#eee
            class A hot
            class B warn
            style C stroke:green";
        let spec = parse_dsl_or_json(dsl, BoxStyle::Rounded).unwrap();
        match spec {
            DiagramSpec::Flowchart(f) => {
                let a = f.nodes.iter().find(|n| n.id == "A").unwrap();
                let b = f.nodes.iter().find(|n| n.id == "B").unwrap();
                let c = f.nodes.iter().find(|n| n.id == "C").unwrap();
                assert_eq!(a.color, Some(Color::Red));
                assert_eq!(b.color, Some(Color::Hex(255, 136, 0)));
                assert_eq!(c.color, Some(Color::Green));
            }
            _ => panic!("Expected flowchart"),
        }
    }

    #[test]
    fn test_parse_link_style_color() {
        let dsl = "graph TB; A --> B; B --> C; linkStyle 1 stroke:red";
        let spec = parse_dsl_or_json(dsl, BoxStyle::Rounded).unwrap();
        match spec {
            DiagramSpec::Flowchart(f) => {
                assert_eq!(f.edges[0].color, None);
                assert_eq!(f.edges[1].color, Some(Color::Red));
            }
            _ => panic!("Expected flowchart"),
        }
    }

    #[test]
    fn test_parse_stroke_width_not_a_color() {
        let dsl = "graph TB; A --> B; style A stroke-width:2px";
        let spec = parse_dsl_or_json(dsl, BoxStyle::Rounded).unwrap();
        match spec {
            DiagramSpec::Flowchart(f) => {
                assert_eq!(f.nodes[0].color, None, "stroke-width is not stroke");
            }
            _ => panic!("Expected flowchart"),
        }
    }

    #[test]
    fn test_parse_class_without_dasharray_ignored() {
        let dsl = "graph TB; A --> B; classDef hot fill:#f00; class A hot";
        let spec = parse_dsl_or_json(dsl, BoxStyle::Rounded).unwrap();
        match spec {
            DiagramSpec::Flowchart(f) => {
                assert_eq!(f.nodes.len(), 2, "class lines must not create nodes");
                assert_eq!(f.edges.len(), 1);
                assert!(!f.nodes.iter().any(|n| n.dashed_border));
            }
            _ => panic!("Expected flowchart"),
        }
    }

    #[test]
    fn test_parse_link_style_fill_alias() {
        let dsl = "graph TB; A --> B; B --> C; linkStyle 1 fill:red";
        match parse_dsl_or_json(dsl, BoxStyle::Rounded).unwrap() {
            DiagramSpec::Flowchart(f) => {
                assert_eq!(
                    f.edges[1].color,
                    Some(Color::Red),
                    "fill aliases to edge color"
                );
            }
            _ => panic!("Expected flowchart"),
        }
    }

    #[test]
    fn test_parse_link_style_dasharray() {
        let dsl = "graph TB; A --> B; B --> C; linkStyle 1 stroke-dasharray: 5";
        let spec = parse_dsl_or_json(dsl, BoxStyle::Rounded).unwrap();
        match spec {
            DiagramSpec::Flowchart(f) => {
                assert!(!f.edges[0].dashed, "edge 0 untouched");
                assert!(f.edges[1].dashed, "linkStyle 1 marks edge 1");
            }
            _ => panic!("Expected flowchart"),
        }
    }

    #[test]
    fn test_parse_subgraphs() {
        let dsl = "graph TB
            subgraph outer [Outer Group]
              A --> B
              subgraph inner
                C
              end
            end
            B --> D";
        let spec = parse_dsl_or_json(dsl, BoxStyle::Rounded).unwrap();
        match spec {
            DiagramSpec::Flowchart(f) => {
                assert_eq!(f.nodes.len(), 4);
                assert_eq!(f.edges.len(), 2, "A-->B inside, B-->D outside");
                assert_eq!(f.subgraphs.len(), 1, "one root subgraph");
                let outer = &f.subgraphs[0];
                assert_eq!(outer.title.as_deref(), Some("Outer Group"));
                assert_eq!(
                    outer.nodes,
                    vec!["A", "B"],
                    "declared-inside members in order"
                );
                assert_eq!(outer.subgraphs.len(), 1, "nested subgraph kept");
                let inner = &outer.subgraphs[0];
                assert_eq!(inner.title.as_deref(), Some("inner"));
                assert_eq!(inner.nodes, vec!["C"]);
                assert!(inner.subgraphs.is_empty());
            }
            _ => panic!("Expected flowchart"),
        }
    }

    #[test]
    fn test_parse_subgraph_direction() {
        let dsl = "graph TB
            A --> B
            subgraph inner [Inner]
              direction LR
              X --> Y
            end";
        let spec = parse_dsl_or_json(dsl, BoxStyle::Rounded).unwrap();
        match spec {
            DiagramSpec::Flowchart(f) => {
                assert_eq!(f.direction, LayoutDirection::TB);
                let inner = &f.subgraphs[0];
                assert_eq!(inner.direction, Some(LayoutDirection::LR));
            }
            _ => panic!("Expected flowchart"),
        }
    }

    #[test]
    fn test_parse_direction_outside_subgraph_sets_global() {
        let dsl = "flowchart TD\n            direction LR\n            A --> B";
        let spec = parse_dsl_or_json(dsl, BoxStyle::Rounded).unwrap();
        match spec {
            DiagramSpec::Flowchart(f) => {
                assert_eq!(f.direction, LayoutDirection::LR);
                assert!(f.subgraphs.is_empty());
            }
            _ => panic!("Expected flowchart"),
        }
    }

    #[test]
    fn test_parse_subgraph_preexisting_node_not_member() {
        let dsl = "graph TB; A --> B; subgraph g; B --> C; end";
        let spec = parse_dsl_or_json(dsl, BoxStyle::Rounded).unwrap();
        match spec {
            DiagramSpec::Flowchart(f) => {
                let g = &f.subgraphs[0];
                assert_eq!(g.nodes, vec!["C"], "B existed before the block");
            }
            _ => panic!("Expected flowchart"),
        }
    }

    #[test]
    fn test_parse_tree_node_color() {
        let dsl = "Root\n    Server @red\n    API (port 8080) @#3498db\n    user@host";
        let spec = parse_dsl_or_json(dsl, BoxStyle::Rounded).unwrap();
        match spec {
            DiagramSpec::Tree(t) => {
                assert_eq!(t.root.name, "Root");
                let kids = &t.root.children;
                assert_eq!(kids[0].name, "Server");
                assert_eq!(kids[0].color, Some(Color::Red));
                assert_eq!(kids[1].name, "API");
                assert_eq!(kids[1].annotation.as_deref(), Some("port 8080"));
                assert_eq!(kids[1].color, Some(Color::parse("#3498db").unwrap()));
                // `user@host` is not a color tag — label stays intact
                assert_eq!(kids[2].name, "user@host");
                assert!(kids[2].color.is_none());
            }
            _ => panic!("Expected tree"),
        }
    }

    #[test]
    fn test_parse_stack_layer_color() {
        let dsl = "memory-map Firmware\n    0xFFFF: ISR vector @red\n    0x8000: App (main) @blue\n    0x0000:";
        let spec = parse_dsl_or_json(dsl, BoxStyle::Rounded).unwrap();
        match spec {
            DiagramSpec::Stack(s) => {
                assert_eq!(s.layers[0].label, "ISR vector");
                assert_eq!(s.layers[0].color, Some(Color::Red));
                assert_eq!(s.layers[1].label, "App");
                assert_eq!(s.layers[1].description.as_deref(), Some("main"));
                assert_eq!(s.layers[1].color, Some(Color::Blue));
                assert_eq!(s.bottom_address.as_deref(), Some("0x0000"));
            }
            _ => panic!("Expected stack"),
        }
    }

    #[test]
    fn test_parse_table_color_directive() {
        let dsl = "table\ncolor: #e74c3c\n| A | B |\n|---|---|\n| 1 | 2 |";
        let spec = parse_dsl_or_json(dsl, BoxStyle::Rounded).unwrap();
        match spec {
            DiagramSpec::Table(t) => {
                assert_eq!(t.color, Some(Color::parse("#e74c3c").unwrap()));
                assert_eq!(t.headers, vec!["A", "B"]);
                assert_eq!(t.rows, vec![vec!["1", "2"]]);
            }
            _ => panic!("Expected table"),
        }
    }

    #[test]
    fn test_parse_fill_and_stroke_width() {
        let dsl = [
            "graph TB; A[X]; B[Y]; C[Z]",
            "classDef hot fill:red,stroke:#c0392b",
            "classDef heavy stroke-width:3px",
            "class A hot",
            "style B fill:blue",
            "style C stroke-width:1px",
        ]
        .join("\n");
        let spec = parse_dsl_or_json(&dsl, BoxStyle::Rounded).unwrap();
        match spec {
            DiagramSpec::Flowchart(f) => {
                let node = |id: &str| f.nodes.iter().find(|n| n.id == id).unwrap();
                let a = node("A");
                assert_eq!(a.fill_color, Some(Color::Red));
                assert_eq!(a.color, Some(Color::parse("#c0392b").unwrap()));
                assert_eq!(a.border_level, 0);
                let b = node("B");
                assert_eq!(b.fill_color, Some(Color::Blue));
                let c = node("C");
                assert_eq!(c.border_level, 0, "stroke-width:1px stays default");
            }
            _ => panic!("Expected flowchart"),
        }
    }

    #[test]
    fn test_parse_stroke_width_thick() {
        let dsl = "graph TB; A[X]\nstyle A stroke-width:2px";
        match parse_dsl_or_json(dsl, BoxStyle::Rounded).unwrap() {
            DiagramSpec::Flowchart(f) => {
                assert_eq!(f.nodes[0].border_level, 1, "2px = heavy");
            }
            _ => panic!("Expected flowchart"),
        }
    }

    #[test]
    fn test_parse_stroke_width_double() {
        let dsl = "graph TB; A[X]\nstyle A stroke-width:3px";
        match parse_dsl_or_json(dsl, BoxStyle::Rounded).unwrap() {
            DiagramSpec::Flowchart(f) => {
                assert_eq!(f.nodes[0].border_level, 2, "3px = double");
            }
            _ => panic!("Expected flowchart"),
        }
    }

    #[test]
    fn test_parse_mermaid_sequence() {
        let dsl = r"
        sequenceDiagram
          participant C as Client
          participant S as Server
          C -> S: Ping
          S --> C: Pong
        ";

        let spec = parse_dsl_or_json(dsl, BoxStyle::Rounded).unwrap();
        match spec {
            DiagramSpec::Sequence(s) => {
                assert_eq!(s.participants.len(), 2);
                assert_eq!(s.messages.len(), 2);
                assert_eq!(s.messages[0].label, "Ping");
            }
            _ => panic!("Expected sequence"),
        }
    }

    #[test]
    fn test_parse_sequence_frames() {
        let dsl = r"
        sequenceDiagram
          A->>B: ping
          alt ok
            B-->>A: pong
          else bad
            B-->>A: error
          end
          loop every 1s
            A->>B: tick
          end
        ";
        let spec = match parse_sequence_dsl(dsl, BoxStyle::Rounded).unwrap() {
            DiagramSpec::Sequence(s) => s,
            _ => panic!("Expected sequence"),
        };
        assert_eq!(spec.messages.len(), 4);
        assert_eq!(spec.frames.len(), 2);

        let alt = &spec.frames[0];
        assert_eq!(alt.label, "alt");
        assert_eq!(alt.branches, vec!["ok", "bad"]);
        assert_eq!(alt.start_step, 1);
        assert_eq!(alt.branch_steps, vec![1, 2]);
        assert_eq!(alt.end_step, 3);

        let lp = &spec.frames[1];
        assert_eq!(lp.label, "loop");
        assert_eq!(lp.branches, vec!["every 1s"]);
        assert_eq!(lp.start_step, 3);
        assert_eq!(lp.end_step, 4);
    }

    fn ds_render(spec: &DiagramSpec) -> String {
        crate::render_diagram(spec)
    }

    #[test]
    fn test_ds_tree_shorthand_matches_json_render() {
        let dsl_spec = parse_dsl_or_json("ds tree 8 3 10 1", BoxStyle::Rounded).unwrap();
        let json_spec = parse_dsl_or_json(
            r#"{"type":"datastructure","kind":"tree","values":["8","3","10","1"]}"#,
            BoxStyle::Rounded,
        )
        .unwrap();
        // Same spec shape, byte-identical render.
        let DiagramSpec::DataStructure(ds) = &dsl_spec else {
            panic!("Expected datastructure")
        };
        assert_eq!(ds.values, vec!["8", "3", "10", "1"]);
        assert_eq!(ds_render(&dsl_spec), ds_render(&json_spec));
    }

    #[test]
    fn test_ds_btree_shorthand_bfs_children() {
        let spec = match parse_dsl_or_json("ds btree 10,20 | 3,5 12,15 25,30", BoxStyle::Rounded)
            .unwrap()
        {
            DiagramSpec::DataStructure(ds) => ds,
            other => panic!("Expected datastructure, got {other:?}"),
        };
        let root = spec.btree_root.as_ref().unwrap();
        assert_eq!(root.keys, vec!["10".to_string(), "20".to_string()]);
        let child_keys: Vec<_> = root.children.iter().map(|c| c.keys.clone()).collect();
        assert_eq!(
            child_keys,
            vec![
                vec!["3".to_string(), "5".to_string()],
                vec!["12".to_string(), "15".to_string()],
                vec!["25".to_string(), "30".to_string()],
            ]
        );
        let out = ds_render(
            &parse_dsl_or_json("ds btree 10,20 | 3,5 12,15 25,30", BoxStyle::Sharp).unwrap(),
        );
        assert!(out.contains("│ 10 │ 20 │"));
        assert!(out.contains("│ 25 │ 30 │"));
    }

    #[test]
    fn test_ds_btree_matches_json_render() {
        let dsl_spec =
            parse_dsl_or_json("ds btree 10,20 | 3,5 12,15 25,30", BoxStyle::Rounded).unwrap();
        let json_spec = parse_dsl_or_json(
            r#"{"type":"datastructure","kind":"btree","btree_root":{"keys":["10","20"],"children":[{"keys":["3","5"]},{"keys":["12","15"]},{"keys":["25","30"]}]}}"#,
            BoxStyle::Rounded,
        )
        .unwrap();
        assert_eq!(ds_render(&dsl_spec), ds_render(&json_spec));
    }

    #[test]
    fn test_ds_btree_permissive_arity() {
        // Root has 2 keys (3 child slots) but only 1 cell: renders as-is.
        let spec = parse_dsl_or_json("ds btree 10,20 | 3,5", BoxStyle::Rounded).unwrap();
        let out = ds_render(&spec);
        assert!(out.contains("10"));
        assert!(out.contains("3"));
    }

    #[test]
    fn test_ds_btree_three_levels() {
        let spec =
            match parse_dsl_or_json("ds btree 10 | 3 20 | 1 7 15 30", BoxStyle::Rounded).unwrap() {
                DiagramSpec::DataStructure(ds) => ds,
                other => panic!("Expected datastructure, got {other:?}"),
            };
        let root = spec.btree_root.as_ref().unwrap();
        assert_eq!(root.keys, vec!["10".to_string()]);
        assert_eq!(root.children.len(), 2);
        assert_eq!(root.children[0].children.len(), 2);
        assert_eq!(root.children[1].children.len(), 2);
        assert_eq!(root.children[1].children[1].keys, vec!["30".to_string()]);
    }

    #[test]
    fn test_ds_btree_extra_cells_nest_deeper() {
        // Permissive arity cuts both ways: 1-key root takes 2 cells, the
        // third cell fills the first child's second slot (BFS order).
        let spec = match parse_dsl_or_json("ds btree 10 | 3 5 7", BoxStyle::Rounded).unwrap() {
            DiagramSpec::DataStructure(ds) => ds,
            other => panic!("Expected datastructure, got {other:?}"),
        };
        let root = spec.btree_root.as_ref().unwrap();
        assert_eq!(root.children.len(), 2);
        assert_eq!(root.children[0].children.len(), 1);
        assert_eq!(root.children[0].children[0].keys, vec!["7".to_string()]);
        assert_eq!(root.children[1].children.len(), 0);
    }

    #[test]
    fn test_ds_malformed_errors() {
        for (input, expect) in [
            ("ds", "expected `ds tree"),
            ("ds heap 1 2", "unknown `ds` kind"),
            ("ds tree", "needs at least one value"),
            ("ds btree", "needs a root key list"),
            ("ds btree ,", "empty root key"),
            ("ds btree 10, | 3", "empty root key"),
            ("ds btree 10 | 3,,5", "empty level cell"),
        ] {
            let err = parse_dsl_or_json(input, BoxStyle::Rounded)
                .err()
                .unwrap_or_else(|| panic!("`{input}` should fail"));
            assert!(err.contains(expect), "`{input}`: {err}");
            // Every error names the expected syntax so it is actionable.
            assert!(err.contains("ds btree 10,20"), "`{input}`: {err}");
        }
    }

    #[test]
    fn test_datastructure_keyword_still_json_hint() {
        let err = parse_dsl_or_json("datastructure", BoxStyle::Rounded)
            .expect_err("bare `datastructure` stays a JSON hint");
        assert!(err.contains("JSON-only"));
    }

    #[test]
    fn test_parse_sequence_notes() {
        let dsl = r"
        sequenceDiagram
          A->>B: ping
          note over A: self note
          note over A,B: cross note
          note right of B: right note
          note left of A: left note
        ";
        let spec = match parse_sequence_dsl(dsl, BoxStyle::Rounded).unwrap() {
            DiagramSpec::Sequence(s) => s,
            _ => panic!("Expected sequence"),
        };
        assert_eq!(spec.notes.len(), 4);
        assert_eq!(spec.notes[0].over, vec!["A"]);
        assert_eq!(spec.notes[0].text, "self note");
        assert_eq!(spec.notes[0].at_step, 1);
        assert_eq!(spec.notes[0].position, SeqNotePosition::Over);

        assert_eq!(spec.notes[1].over, vec!["A", "B"]);
        assert_eq!(spec.notes[1].text, "cross note");
        assert_eq!(spec.notes[1].at_step, 1);
        assert_eq!(spec.notes[1].position, SeqNotePosition::Over);

        assert_eq!(spec.notes[2].over, vec!["B"]);
        assert_eq!(spec.notes[2].text, "right note");
        assert_eq!(spec.notes[2].at_step, 1);
        assert_eq!(spec.notes[2].position, SeqNotePosition::RightOf);

        assert_eq!(spec.notes[3].over, vec!["A"]);
        assert_eq!(spec.notes[3].text, "left note");
        assert_eq!(spec.notes[3].at_step, 1);
        assert_eq!(spec.notes[3].position, SeqNotePosition::LeftOf);
    }

    #[test]
    fn test_parse_sequence_note_introduces_participant() {
        let dsl = r"
        sequenceDiagram
          Note over C: note before message
          A->>B: hi
        ";
        let spec = match parse_sequence_dsl(dsl, BoxStyle::Rounded).unwrap() {
            DiagramSpec::Sequence(s) => s,
            _ => panic!("Expected sequence"),
        };
        assert_eq!(spec.participants.len(), 3);
        assert_eq!(spec.participants[0].id, "C");
        assert_eq!(spec.notes.len(), 1);
        assert_eq!(spec.notes[0].at_step, 0);
        assert_eq!(spec.notes[0].text, "note before message");
    }

    #[test]
    fn test_parse_sequence_unrecognized_diagnostics() {
        // (a) header + 'foo','bar' -> Err naming lines 2,3
        let bad = "sequenceDiagram\nfoo\nbar";
        let err = parse_sequence_dsl(bad, BoxStyle::Rounded).unwrap_err();
        assert!(err.contains("line 2: `foo`"), "err: {err}");
        assert!(err.contains("line 3: `bar`"), "err: {err}");

        // (b) junk line + valid message -> Ok 1 message (lenient best-effort)
        let tolerant = "sequenceDiagram\njunk\nA->>B: hi";
        let spec = match parse_sequence_dsl(tolerant, BoxStyle::Rounded).unwrap() {
            DiagramSpec::Sequence(s) => s,
            _ => panic!("Expected sequence"),
        };
        assert_eq!(spec.messages.len(), 1);

        // (c) %% comment not flagged
        let commented = "sequenceDiagram\n%% comment\nA->>B: hi";
        assert!(parse_sequence_dsl(commented, BoxStyle::Rounded).is_ok());

        // (d) bare sequenceDiagram still Ok empty
        let bare = "sequenceDiagram";
        let spec = match parse_sequence_dsl(bare, BoxStyle::Rounded).unwrap() {
            DiagramSpec::Sequence(s) => s,
            _ => panic!("Expected sequence"),
        };
        assert!(spec.messages.is_empty());
    }

    #[test]
    fn test_parse_sequence_frame_autocommit_eof() {
        // (a) unclosed alt -> 1 frame end_step==1 branches==['ok']
        let dsl = "sequenceDiagram\nalt ok\nA->>B: msg";
        let spec = match parse_sequence_dsl(dsl, BoxStyle::Rounded).unwrap() {
            DiagramSpec::Sequence(s) => s,
            _ => panic!("Expected sequence"),
        };
        assert_eq!(spec.frames.len(), 1);
        assert_eq!(spec.frames[0].label, "alt");
        assert_eq!(spec.frames[0].branches, vec!["ok"]);
        assert_eq!(spec.frames[0].end_step, 1);

        // (b) nested unclosed loop > alt -> 2 frames inner index 0
        let nested = "sequenceDiagram\nloop every 1s\nalt inner\nA->>B: msg";
        let n_spec = match parse_sequence_dsl(nested, BoxStyle::Rounded).unwrap() {
            DiagramSpec::Sequence(s) => s,
            _ => panic!("Expected sequence"),
        };
        assert_eq!(n_spec.frames.len(), 2);
        assert_eq!(n_spec.frames[0].label, "alt");
        assert_eq!(n_spec.frames[1].label, "loop");

        // (c) unclosed break -> frame labeled break
        let brk = "sequenceDiagram\nbreak timeout\nA->>B: msg";
        let b_spec = match parse_sequence_dsl(brk, BoxStyle::Rounded).unwrap() {
            DiagramSpec::Sequence(s) => s,
            _ => panic!("Expected sequence"),
        };
        assert_eq!(b_spec.frames.len(), 1);
        assert_eq!(b_spec.frames[0].label, "break");
        assert_eq!(b_spec.frames[0].branches, vec!["timeout"]);

        // (d) closed input identical spec
        let closed = "sequenceDiagram\nalt ok\nA->>B: msg\nend";
        let c_spec = match parse_sequence_dsl(closed, BoxStyle::Rounded).unwrap() {
            DiagramSpec::Sequence(s) => s,
            _ => panic!("Expected sequence"),
        };
        assert_eq!(spec.frames[0], c_spec.frames[0]);
    }

    #[test]
    fn test_parse_sequence_critical_option() {
        // (a) critical up / msg / option backup / msg / end
        let dsl = r"
        sequenceDiagram
          critical up
            A->>B: try up
          option backup
            A->>B: try backup
          end
        ";
        let spec = match parse_sequence_dsl(dsl, BoxStyle::Rounded).unwrap() {
            DiagramSpec::Sequence(s) => s,
            _ => panic!("Expected sequence"),
        };
        assert_eq!(spec.frames.len(), 1);
        assert_eq!(spec.frames[0].label, "critical");
        assert_eq!(spec.frames[0].branches, vec!["up", "backup"]);
        assert_eq!(spec.frames[0].branch_steps, vec![0, 1]);

        // (b) option outside critical is unrecognized
        let bad = "sequenceDiagram\noption orphan";
        assert!(parse_sequence_dsl(bad, BoxStyle::Rounded).is_err());

        // (d) nested critical inside alt: option binds innermost
        let nested = r"
        sequenceDiagram
          alt outer
            critical inner
              A->>B: msg1
            option inner_opt
              A->>B: msg2
            end
          else outer_else
            A->>B: msg3
          end
        ";
        let n_spec = match parse_sequence_dsl(nested, BoxStyle::Rounded).unwrap() {
            DiagramSpec::Sequence(s) => s,
            _ => panic!("Expected sequence"),
        };
        assert_eq!(n_spec.frames.len(), 2);
        // inner critical committed on end -> index 0
        assert_eq!(n_spec.frames[0].label, "critical");
        assert_eq!(n_spec.frames[0].branches, vec!["inner", "inner_opt"]);
        // outer alt committed on second end -> index 1
        assert_eq!(n_spec.frames[1].label, "alt");
        assert_eq!(n_spec.frames[1].branches, vec!["outer", "outer_else"]);
    }

    #[test]
    fn test_parse_sequence_header_prefix_strip() {
        // (a) same line message suffix
        let dsl = "sequenceDiagram A->>B: hi";
        let spec = match parse_sequence_dsl(dsl, BoxStyle::Rounded).unwrap() {
            DiagramSpec::Sequence(s) => s,
            _ => panic!("Expected sequence"),
        };
        assert_eq!(spec.participants.len(), 2);
        assert_eq!(spec.messages.len(), 1);
        assert_eq!(spec.messages[0].label, "hi");

        // (b) semicolon delimited direct call
        let dsl_semi = "sequenceDiagram;A->>B: hi";
        let spec_semi = match parse_sequence_dsl(dsl_semi, BoxStyle::Rounded).unwrap() {
            DiagramSpec::Sequence(s) => s,
            _ => panic!("Expected sequence"),
        };
        assert_eq!(spec_semi.messages.len(), 1);

        // (c) bare header line
        assert!(parse_sequence_dsl("sequenceDiagram", BoxStyle::Rounded).is_ok());

        // (d) junk with no arrow fails T-3
        let junk = "sequenceDiagram A";
        let err = parse_sequence_dsl(junk, BoxStyle::Rounded).unwrap_err();
        assert!(err.contains("line 1: `A`"), "err: {err}");
    }
}
