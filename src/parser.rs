use crate::schema::{
    ArrowDirection, DiagramSpec, EdgeSpec, FlowchartSpec, LayoutDirection, NodeShape, NodeSpec,
    ParticipantSpec, SeqFrameSpec, SeqMessageSpec, SeqMessageType, SequenceSpec, StackLayerSpec,
    StackSpec, TableSpec, TextAlign, TreeNodeSpec, TreeSpec,
};
use crate::theme::BoxStyle;

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
    } else if trimmed.contains("-->") || trimmed.contains("->") {
        // Default to flowchart if arrow detected
        parse_flowchart_dsl(trimmed, default_style)
    } else {
        // Fallback to tree
        parse_tree_dsl(trimmed, default_style)
    }
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
            });
        }
    };

    for line in lines {
        let trimmed = line.trim();
        if trimmed.is_empty()
            || trimmed.starts_with("```")
            || trimmed.starts_with("%%")
            || trimmed.starts_with("//")
            || trimmed.starts_with("subgraph")
            || trimmed == "end"
            || trimmed.starts_with("direction")
            || trimmed.starts_with("classDef")
            || trimmed.starts_with("class ")
            || trimmed.starts_with("style ")
        {
            continue;
        }

        if find_next_delim(trimmed).is_some() {
            parse_flowchart_edges_in_line(trimmed, &mut ensure_node, &mut edges);
        } else {
            // Standalone node definition: e.g. A[Label]
            let (id, label, shape) = parse_node_token(trimmed);
            if !id.is_empty() {
                ensure_node(&id, &label, shape);
            }
        }
    }

    Ok(DiagramSpec::Flowchart(FlowchartSpec {
        direction,
        style: default_style,
        title: None,
        nodes,
        edges,
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

    // [Box]
    if let Some(start) = t.find('[')
        && let Some(end) = t.rfind(']')
    {
        let id = t[..start].trim().to_string();
        let label = clean_label(&t[start + 1..end]);
        return (id, label, NodeShape::Box);
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
        return (id, label, NodeShape::Diamond);
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

fn find_next_delim(text: &str) -> Option<(usize, &'static str)> {
    let delimiters = ["<==>", "==>", "<-->", "<--", "-.->", "-->", "---"];
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

fn parse_flowchart_edges_in_line<F>(mut line: &str, ensure_node: &mut F, edges: &mut Vec<EdgeSpec>)
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
            "<-->" | "<==>" => ArrowDirection::Both,
            "<--" => ArrowDirection::Back,
            _ => ArrowDirection::Forward,
        };
        let is_dashed = delim == "-.->";
        let is_thick = delim == "==>" || delim == "<==>";

        let sources = split_bracket_aware(left_part, '&');
        let targets = split_bracket_aware(target_part, '&');

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
                });
            }
        }

        if !has_next {
            break;
        }
        line = rest;
    }
}

/// Parses a Mermaid sequence-diagram DSL into a [`SequenceSpec`].
///
/// # Errors
///
/// Currently always returns `Ok`; the `Result` keeps the parser signatures
/// uniform with the other DSL parsers.
pub fn parse_sequence_dsl(input: &str, default_style: BoxStyle) -> Result<DiagramSpec, String> {
    let mut participants = Vec::new();
    let mut p_set = std::collections::HashSet::new();
    let mut messages = Vec::new();
    let mut frames = Vec::new();
    // Stack of in-progress frames; `end` pops and commits them (supports nesting)
    let mut open_frames: Vec<SeqFrameSpec> = Vec::new();

    let add_participant = |id: &str,
                           label: Option<String>,
                           p_list: &mut Vec<ParticipantSpec>,
                           p_s: &mut std::collections::HashSet<String>| {
        if !p_s.contains(id) {
            p_s.insert(id.to_string());
            p_list.push(ParticipantSpec {
                id: id.to_string(),
                label,
            });
        }
    };

    for line in input.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("%%") || trimmed == "sequenceDiagram" {
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
        if let Some(rest) = trimmed
            .strip_prefix("else ")
            .or_else(|| trimmed.strip_prefix("and "))
        {
            if let Some(frame) = open_frames.last_mut() {
                frame.branches.push(rest.trim().to_string());
                frame.branch_steps.push(messages.len());
            }
            continue;
        }

        // Messages: A ->> B: Msg or A -> B: Msg or A --> B: Msg
        let arrow_patterns = ["-->>", "-->", "<->", "->>", "->"];
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
                break;
            }
        }
    }

    Ok(DiagramSpec::Sequence(SequenceSpec {
        style: default_style,
        title: None,
        participants,
        messages,
        notes: vec![],
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

    for line in input.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("table") {
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
}
