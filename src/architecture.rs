use crate::canvas::{Canvas, Direction, Rect};
use crate::schema::{
    ArchitectureSpec, ContainerItem, ContainerLayout, ContainerSpec, EdgeSpec, LeafComponent,
};
use crate::theme::Theme;
use std::collections::{HashMap, HashSet};
use unicode_width::UnicodeWidthStr;

#[derive(Clone, Debug)]
struct BoxBounds {
    x: usize,
    y: usize,
    width: usize,
    height: usize,
}

pub struct ArchitectureRenderer<'a> {
    spec: &'a ArchitectureSpec,
    theme: Theme,
}

impl<'a> ArchitectureRenderer<'a> {
    #[must_use]
    pub fn new(spec: &'a ArchitectureSpec, theme: Theme) -> Self {
        Self { spec, theme }
    }

    #[must_use]
    #[allow(
        clippy::similar_names,
        reason = "u_/v_ prefixes denote the two endpoints of a connection"
    )]
    pub fn render(&self, colored: bool) -> String {
        if self.spec.containers.is_empty() {
            return String::new();
        }

        let mut comp_bounds: HashMap<String, BoxBounds> = HashMap::new();
        let mut container_ids: HashSet<String> = HashSet::new();

        // Compute layout for each top-level container
        let gap = 4;
        let mut top_layouts = Vec::new();
        let start_y = if self.spec.title.is_some() { 2 } else { 0 };
        let mut cur_y = start_y;

        for c in &self.spec.containers {
            container_ids.insert(c.id.clone());
            let (w, h) = self.measure_container(c);
            top_layouts.push((0, cur_y, w, h));
            cur_y += h + gap;
        }

        let max_w = top_layouts
            .iter()
            .map(|(_, _, w, _)| *w)
            .max()
            .unwrap_or(40);
        let total_h = cur_y + 2;

        let mut canvas = Canvas::new(max_w + 4, total_h);

        // Title
        if let Some(ref title) = self.spec.title {
            let tw = UnicodeWidthStr::width(title.as_str());
            let tx = if max_w > tw { (max_w - tw) / 2 } else { 0 };
            canvas.draw_text(tx, 0, title);
        }

        // Draw containers first: fills comp_bounds and lays down walls
        for (i, c) in self.spec.containers.iter().enumerate() {
            let (_, y, w, h) = top_layouts[i];
            self.render_container(
                &mut canvas,
                c,
                Rect::new(0, y, w, h),
                &mut comp_bounds,
                &mut container_ids,
            );
        }

        let warnings =
            skipped_connection_warnings(&self.spec.connections, &comp_bounds, &container_ids);
        for msg in warnings {
            eprintln!("Warning: architecture: {msg}");
        }

        // Classify top-to-bottom connections that cross intermediate containers
        // (source and target containers separated by at least one other). These
        // route via a shared right-margin corridor track per source instead of
        // slicing straight through the containers in between.
        let container_of = |b: &BoxBounds| -> Option<usize> {
            let cx = b.x + b.width / 2;
            let cy = b.y + b.height / 2;
            top_layouts
                .iter()
                .position(|&(_, ly, lw, lh)| cx < lw && cy >= ly && cy < ly + lh)
        };
        let mut arch_tracks: HashMap<String, (usize, usize)> = HashMap::new();
        let mut next_track = max_w + 2;
        for conn in &self.spec.connections {
            if let (Some(u), Some(v)) = (comp_bounds.get(&conn.from), comp_bounds.get(&conn.to))
                && u.y + u.height <= v.y
                && let (Some(uct), Some(vct)) = (container_of(u), container_of(v))
                && vct >= uct + 2
            {
                let entry_y = top_layouts[vct].1 - 1;
                let entry = arch_tracks.entry(conn.from.clone()).or_insert_with(|| {
                    let t = (next_track, entry_y);
                    next_track += 4;
                    t
                });
                entry.1 = entry.1.max(entry_y);
            }
        }
        let mut arch_led: HashSet<String> = HashSet::new();

        // Draw inter-component connection lines
        for conn in &self.spec.connections {
            if conn.from == conn.to
                || container_ids.contains(&conn.from)
                || container_ids.contains(&conn.to)
            {
                continue;
            }
            canvas.set_pen(conn.color);
            if let (Some(u), Some(v)) = (comp_bounds.get(&conn.from), comp_bounds.get(&conn.to)) {
                let u_right = u.x + u.width - 1;
                let u_cy = u.y + u.height / 2;
                let v_left = v.x;
                let v_cy = v.y + v.height / 2;

                if u_right < v_left {
                    // Left to Right connection
                    let mid_x = u_right + (v_left - u_right) / 2;
                    canvas.draw_hline(u_right + 1, mid_x, u_cy);
                    canvas.draw_vline(mid_x, u_cy, v_cy);
                    canvas.draw_hline(mid_x, v_left - 1, v_cy);
                } else if u.x > v.x + v.width - 1 && u_cy == v_cy {
                    // Right to Left, same row: route below both components
                    let u_cx = u.x + u.width / 2;
                    let v_cx = v.x + v.width / 2;
                    let route_y = u.y + u.height + 1;
                    canvas.draw_vline(u_cx, u.y + u.height, route_y);
                    canvas.draw_hline(v_cx.min(u_cx), v_cx.max(u_cx), route_y);
                    canvas.draw_vline(v_cx, route_y, v.y + v.height);
                } else if u.y + u.height <= v.y {
                    // Top to Bottom connection
                    let u_cx = u.x + u.width / 2;
                    let u_bottom = u.y + u.height - 1;
                    let v_cx = v.x + v.width / 2;
                    let v_top = v.y;

                    let uct = container_of(u);
                    let vct = container_of(v);
                    let crosses = matches!((uct, vct), (Some(a), Some(b)) if b >= a + 2);
                    if crosses
                        && let Some(&(track_x, depth_y)) = arch_tracks.get(conn.from.as_str())
                    {
                        let uct = uct.unwrap_or(0);
                        let exit_y = top_layouts[uct].1 + top_layouts[uct].3;
                        let entry_y = vct.map_or(0, |k| top_layouts[k].1 - 1);

                        let lead = arch_led.insert(conn.from.clone());
                        if lead {
                            // One shared corridor run per source
                            canvas.draw_vline(u_cx, u_bottom + 1, exit_y);
                            canvas.draw_hline(u_cx, track_x, exit_y);
                            canvas.draw_vline(track_x, exit_y, depth_y);
                        }
                        // Drop into the target container's corridor
                        canvas.draw_hline(track_x, v_cx, entry_y);
                        if entry_y < v_top - 1 {
                            canvas.draw_vline(v_cx, entry_y, v_top - 1);
                        }
                    } else {
                        let mid_y = u_bottom + (v_top - u_bottom) / 2;
                        canvas.draw_vline(u_cx, u_bottom + 1, mid_y);
                        canvas.draw_hline(u_cx, v_cx, mid_y);
                        canvas.draw_vline(v_cx, mid_y, v_top - 1);
                    }
                }
            }
        }

        // Draw arrowheads and labels on top of everything so callouts are
        // never buried by lines or container walls
        for conn in &self.spec.connections {
            if conn.from == conn.to
                || container_ids.contains(&conn.from)
                || container_ids.contains(&conn.to)
            {
                continue;
            }
            if let (Some(u), Some(v)) = (comp_bounds.get(&conn.from), comp_bounds.get(&conn.to)) {
                let u_right = u.x + u.width - 1;
                let u_cy = u.y + u.height / 2;
                let v_left = v.x;
                let v_cy = v.y + v.height / 2;

                if u_right < v_left {
                    // Left to Right connection
                    let _mid_x = u_right + (v_left - u_right) / 2;
                    canvas.draw_arrow(v_left - 1, v_cy, Direction::Right, &self.theme);

                    if let Some(ref lbl) = conn.label {
                        let lbl_w = UnicodeWidthStr::width(lbl.as_str());
                        let channel = v_left.saturating_sub(u_right + 1);
                        if channel >= lbl_w {
                            let lx = u_right + 1 + (channel - lbl_w) / 2;
                            let (cx, cy, cw, _) =
                                container_of(u).map_or((0, 0, max_w, total_h), |k| top_layouts[k]);
                            let bounds =
                                Rect::new(cx + 1, cy + 1, cw.saturating_sub(2), usize::MAX);
                            Self::place_label(
                                &mut canvas,
                                bounds,
                                lx,
                                u_cy.saturating_sub(1),
                                &[],
                                lbl,
                            );
                        }
                    }
                } else if u.x > v.x + v.width - 1 && u_cy == v_cy {
                    // Right to Left, same row (routed below)
                    let u_cx = u.x + u.width / 2;
                    let v_cx = v.x + v.width / 2;
                    let route_y = u.y + u.height + 1;
                    canvas.draw_arrow(v_cx, v.y + v.height, Direction::Up, &self.theme);

                    if let Some(ref lbl) = conn.label {
                        let lbl_w = UnicodeWidthStr::width(lbl.as_str());
                        let mid = usize::midpoint(v_cx, u_cx);
                        if lbl_w + 2 < u_cx.saturating_sub(v_cx) {
                            let (cx, cy, cw, ch) =
                                container_of(u).map_or((0, 0, max_w, total_h), |k| top_layouts[k]);
                            let bounds = Rect::new(
                                cx + 1,
                                cy + 1,
                                cw.saturating_sub(2),
                                ch.saturating_sub(2),
                            );
                            let clamps = [
                                (u_cx + 2, route_y),
                                (v_cx.saturating_sub(lbl_w + 2), route_y),
                            ];
                            Self::place_label(
                                &mut canvas,
                                bounds,
                                mid.saturating_sub(lbl_w / 2),
                                route_y + 1,
                                &clamps,
                                lbl,
                            );
                        }
                    }
                } else if u.y + u.height <= v.y {
                    // Top to Bottom connection
                    let u_cx = u.x + u.width / 2;
                    let v_cx = v.x + v.width / 2;
                    let v_top = v.y;
                    let u_bottom = u.y + u.height - 1;
                    let mid_y = u_bottom + (v_top - u_bottom) / 2;
                    canvas.draw_arrow(v_cx, v_top - 1, Direction::Down, &self.theme);

                    let uct = container_of(u);
                    let vct = container_of(v);
                    let crosses = matches!((uct, vct), (Some(a), Some(b)) if b >= a + 2);
                    if crosses && let Some(&(track_x, _)) = arch_tracks.get(conn.from.as_str()) {
                        let entry_y = vct.map_or(0, |k| top_layouts[k].1 - 1);
                        if let Some(ref lbl) = conn.label {
                            let lbl_w = UnicodeWidthStr::width(lbl.as_str());
                            let lx = usize::midpoint(track_x, v_cx)
                                .saturating_sub(lbl_w / 2)
                                .max(v_cx.min(track_x) + 1);
                            canvas.draw_text_safe(lx, entry_y.saturating_sub(1), lbl);
                        }
                    } else if let Some(ref lbl) = conn.label {
                        let (cx, cy, cw, _) =
                            uct.map_or((0, 0, max_w, total_h), |k| top_layouts[k]);
                        let bounds = Rect::new(cx + 1, cy + 1, cw.saturating_sub(2), usize::MAX);
                        let clamp = [(u_cx.max(v_cx) + 2, mid_y)];
                        Self::place_label(
                            &mut canvas,
                            bounds,
                            u_cx + 2,
                            mid_y.saturating_sub(1),
                            &clamp,
                            lbl,
                        );
                    }
                }
            }
        }
        canvas.set_pen(None);

        canvas.render_impl(&self.theme, colored)
    }
    fn calculate_row_gap(&self) -> usize {
        let max_label_w = self
            .spec
            .connections
            .iter()
            .filter_map(|conn| conn.label.as_deref())
            .map(UnicodeWidthStr::width)
            .max()
            .unwrap_or(0);
        (max_label_w + 6).max(16)
    }

    fn measure_leaf(leaf: &LeafComponent) -> (usize, usize) {
        let name_w = UnicodeWidthStr::width(leaf.name.as_str());
        let props_w = leaf
            .properties
            .iter()
            .map(|(k, v)| UnicodeWidthStr::width(format!("{k}: {v}").as_str()))
            .max()
            .unwrap_or(0);

        let w = name_w.max(props_w) + 4; // 2 padding + 2 border
        let h = if leaf.properties.is_empty() {
            3
        } else {
            4 + leaf.properties.len()
        };
        (w.max(14), h)
    }

    fn measure_container(&self, c: &ContainerSpec) -> (usize, usize) {
        let title_w = UnicodeWidthStr::width(c.title.as_str()) + 6;

        if c.items.is_empty() {
            return (title_w.max(16), 4);
        }

        let child_sizes: Vec<(usize, usize)> = c
            .items
            .iter()
            .map(|item| match item {
                ContainerItem::Leaf(l) => Self::measure_leaf(l),
                ContainerItem::SubContainer(sub) => self.measure_container(sub),
            })
            .collect();

        match c.layout {
            ContainerLayout::Row => {
                let gap = self.calculate_row_gap();
                let total_w: usize = child_sizes.iter().map(|(w, _)| *w).sum::<usize>()
                    + gap * (child_sizes.len() - 1)
                    + 4;
                let max_h = child_sizes.iter().map(|(_, h)| *h).max().unwrap_or(3) + 4;
                (total_w.max(title_w), max_h)
            }
            ContainerLayout::Column => {
                let gap = 2;
                let max_w = child_sizes.iter().map(|(w, _)| *w).max().unwrap_or(12) + 4;
                let total_h: usize = child_sizes.iter().map(|(_, h)| *h).sum::<usize>()
                    + gap * (child_sizes.len() - 1)
                    + 4;
                (max_w.max(title_w), total_h)
            }
        }
    }

    fn place_label(
        canvas: &mut Canvas,
        bounds: Rect,
        px: usize,
        py: usize,
        clamps: &[(usize, usize)],
        label: &str,
    ) -> bool {
        if let Some((sx, sy)) = canvas.find_safe_text_pos_within(bounds, px, py, label) {
            canvas.draw_text(sx, sy, label);
            return true;
        }
        let text_w = UnicodeWidthStr::width(label);
        let max_x = bounds.x.saturating_add(bounds.width);
        let max_y = bounds.y.saturating_add(bounds.height);
        for &(cx, cy) in clamps {
            let in_bounds = cy >= bounds.y
                && cy < max_y
                && cx >= bounds.x
                && cx.saturating_add(text_w) <= max_x;
            if in_bounds && canvas.can_place_text(cx, cy, label) {
                canvas.draw_text(cx, cy, label);
                return true;
            }
        }
        false
    }

    fn render_container(
        &self,
        canvas: &mut Canvas,
        c: &ContainerSpec,
        area: Rect,
        bounds: &mut HashMap<String, BoxBounds>,
        container_ids: &mut HashSet<String>,
    ) {
        container_ids.insert(c.id.clone());
        // Draw outer container box
        canvas.set_pen(c.color);
        canvas.draw_box(
            area.x,
            area.y,
            area.width,
            area.height,
            &self.theme,
            Some(&c.title),
        );
        canvas.obstacles.pop();
        canvas.set_pen(None);

        let gap = match c.layout {
            ContainerLayout::Row => self.calculate_row_gap(),
            ContainerLayout::Column => 2,
        };
        let mut cur_x = area.x + 2;
        let mut cur_y = area.y + 2;

        for item in &c.items {
            match item {
                ContainerItem::Leaf(leaf) => {
                    let (w, h) = Self::measure_leaf(leaf);
                    self.render_leaf(canvas, leaf, cur_x, cur_y, w, h);
                    bounds.insert(
                        leaf.id.clone(),
                        BoxBounds {
                            x: cur_x,
                            y: cur_y,
                            width: w,
                            height: h,
                        },
                    );

                    match c.layout {
                        ContainerLayout::Row => {
                            cur_x += w + gap;
                        }
                        ContainerLayout::Column => {
                            cur_y += h + gap;
                        }
                    }
                }
                ContainerItem::SubContainer(sub) => {
                    let (w, h) = self.measure_container(sub);
                    self.render_container(
                        canvas,
                        sub,
                        Rect::new(cur_x, cur_y, w, h),
                        bounds,
                        container_ids,
                    );
                    bounds.insert(
                        sub.id.clone(),
                        BoxBounds {
                            x: cur_x,
                            y: cur_y,
                            width: w,
                            height: h,
                        },
                    );

                    match c.layout {
                        ContainerLayout::Row => {
                            cur_x += w + gap;
                        }
                        ContainerLayout::Column => {
                            cur_y += h + gap;
                        }
                    }
                }
            }
        }
    }

    fn render_leaf(
        &self,
        canvas: &mut Canvas,
        leaf: &LeafComponent,
        x: usize,
        y: usize,
        width: usize,
        height: usize,
    ) {
        canvas.set_pen(leaf.color);
        canvas.draw_box(x, y, width, height, &self.theme, None);
        canvas.add_obstacle(Rect::new(x, y, width, height));
        canvas.set_pen(None);

        // Name
        let name_w = UnicodeWidthStr::width(leaf.name.as_str());
        let name_x = x + (width.saturating_sub(name_w)) / 2;
        canvas.draw_text(name_x, y + 1, &leaf.name);

        // Properties with divider if present
        if !leaf.properties.is_empty() && height >= 4 {
            let right = x + width - 1;
            canvas.draw_hline(x + 1, right - 1, y + 2);
            canvas.draw_corner(
                x,
                y + 2,
                crate::canvas::LineConn {
                    north: true,
                    south: true,
                    east: true,
                    west: false,
                },
            );
            canvas.draw_corner(
                right,
                y + 2,
                crate::canvas::LineConn {
                    north: true,
                    south: true,
                    west: true,
                    east: false,
                },
            );

            for (i, (k, v)) in leaf.properties.iter().enumerate() {
                let prop_text = format!("{k}: {v}");
                canvas.draw_text(x + 2, y + 3 + i, &prop_text);
            }
        } else {
            for (i, (k, v)) in leaf.properties.iter().enumerate() {
                let prop_text = format!("{k}: {v}");
                canvas.draw_text(x + 2, y + 2 + i, &prop_text);
            }
        }
    }
}

fn skipped_connection_warnings(
    connections: &[EdgeSpec],
    bounds: &HashMap<String, BoxBounds>,
    container_ids: &HashSet<String>,
) -> Vec<String> {
    let mut warnings = Vec::new();
    let mut seen: HashSet<(&'static str, String)> = HashSet::new();

    for conn in connections {
        if conn.from == conn.to {
            if seen.insert(("self-loop", conn.from.clone())) {
                warnings.push(format!("skipping self-loop connection on '{}'", conn.from));
            }
            continue;
        }

        for endpoint in [&conn.from, &conn.to] {
            if container_ids.contains(endpoint) {
                if seen.insert(("container-id", (*endpoint).clone())) {
                    warnings.push(format!(
                        "skipping connection referencing container ID '{endpoint}'"
                    ));
                }
            } else if !bounds.contains_key(endpoint)
                && seen.insert(("unknown-id", (*endpoint).clone()))
            {
                warnings.push(format!(
                    "skipping connection with unknown component ID '{endpoint}'"
                ));
            }
        }
    }

    warnings
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::*;
    use crate::theme::BoxStyle;

    #[test]
    fn test_architecture_diagram() {
        let spec = ArchitectureSpec {
            style: BoxStyle::Rounded,
            title: Some("Kubernetes Cluster".to_string()),
            containers: vec![ContainerSpec {
                id: "k8s".to_string(),
                title: "Namespace: Production".to_string(),
                layout: ContainerLayout::Row,
                color: None,
                items: vec![
                    ContainerItem::Leaf(LeafComponent {
                        id: "frontend".to_string(),
                        name: "Frontend".to_string(),
                        properties: vec![("Port".to_string(), "80".to_string())],
                        color: None,
                    }),
                    ContainerItem::Leaf(LeafComponent {
                        id: "backend".to_string(),
                        name: "Backend API".to_string(),
                        properties: vec![("Port".to_string(), "8080".to_string())],
                        color: None,
                    }),
                ],
            }],
            connections: vec![EdgeSpec {
                from: "frontend".to_string(),
                to: "backend".to_string(),
                label: Some("HTTP".to_string()),
                arrow: crate::schema::ArrowDirection::Forward,
                dashed: false,
                thick: false,
                color: None,
            }],
        };

        let renderer = ArchitectureRenderer::new(&spec, Theme::new(BoxStyle::Rounded));
        let out = renderer.render(false);
        println!("ARCHITECTURE OUTPUT:\n{out}");
        assert!(out.contains("Kubernetes Cluster"));
        assert!(out.contains("Namespace: Production"));
        assert!(out.contains("Frontend"));
        assert!(out.contains("Backend API"));
        assert!(out.contains("Port: 80"));
        assert!(out.contains("►"));
    }

    #[test]
    fn test_lr_label_stays_inside_container() {
        let make_spec = |label: Option<String>| ArchitectureSpec {
            style: BoxStyle::Rounded,
            title: Some("Service Mesh".to_string()),
            containers: vec![ContainerSpec {
                id: "c1".to_string(),
                title: "Cluster".to_string(),
                layout: ContainerLayout::Row,
                color: None,
                items: vec![
                    ContainerItem::Leaf(LeafComponent {
                        id: "a".to_string(),
                        name: "Alpha".to_string(),
                        properties: vec![],
                        color: None,
                    }),
                    ContainerItem::Leaf(LeafComponent {
                        id: "b".to_string(),
                        name: "Beta".to_string(),
                        properties: vec![],
                        color: None,
                    }),
                ],
            }],
            connections: vec![EdgeSpec {
                from: "a".to_string(),
                to: "b".to_string(),
                label,
                arrow: ArrowDirection::Forward,
                dashed: false,
                thick: false,
                color: None,
            }],
        };

        let with_lbl = ArchitectureRenderer::new(
            &make_spec(Some("HTTP".to_string())),
            Theme::new(BoxStyle::Rounded),
        )
        .render(false);
        let no_lbl = ArchitectureRenderer::new(&make_spec(None), Theme::new(BoxStyle::Rounded))
            .render(false);

        // Label appears exactly once
        assert_eq!(with_lbl.matches("HTTP").count(), 1);

        let lines_with: Vec<&str> = with_lbl.lines().collect();
        let lines_no: Vec<&str> = no_lbl.lines().collect();
        assert_eq!(lines_with.len(), lines_no.len());

        let top_border_idx = lines_with
            .iter()
            .position(|l| l.contains("Cluster"))
            .unwrap();
        let lbl_idx = lines_with.iter().position(|l| l.contains("HTTP")).unwrap();
        let bottom_border_idx = lines_with
            .iter()
            .rposition(|l| l.contains('╰') && l.contains('╯'))
            .unwrap();

        // Label must be strictly inside container bounds
        assert!(lbl_idx > top_border_idx);
        assert!(lbl_idx < bottom_border_idx);

        // Title line untouched
        assert_eq!(lines_with[top_border_idx], lines_no[top_border_idx]);

        // Glyph diff: difference only on the label line
        for (i, (w, n)) in lines_with.iter().zip(lines_no.iter()).enumerate() {
            if i == lbl_idx {
                assert_ne!(w, n);
            } else {
                assert_eq!(w, n, "line {i} differs unexpectedly");
            }
        }
    }

    #[test]
    fn test_label_never_escapes_above_container() {
        let spec_with_lbl = ArchitectureSpec {
            style: BoxStyle::Rounded,
            title: Some("T".to_string()),
            containers: vec![ContainerSpec {
                id: "c1".to_string(),
                title: "ColumnContainer".to_string(),
                layout: ContainerLayout::Column,
                color: None,
                items: vec![
                    ContainerItem::Leaf(LeafComponent {
                        id: "alpha".to_string(),
                        name: "Alpha".to_string(),
                        properties: vec![],
                        color: None,
                    }),
                    ContainerItem::Leaf(LeafComponent {
                        id: "beta".to_string(),
                        name: "Beta".to_string(),
                        properties: vec![],
                        color: None,
                    }),
                ],
            }],
            connections: vec![EdgeSpec {
                from: "alpha".to_string(),
                to: "beta".to_string(),
                label: Some("HTTP".to_string()),
                arrow: ArrowDirection::Forward,
                dashed: false,
                thick: false,
                color: None,
            }],
        };

        let mut spec_no_lbl = spec_with_lbl.clone();
        spec_no_lbl.connections[0].label = None;

        let out_with =
            ArchitectureRenderer::new(&spec_with_lbl, Theme::new(BoxStyle::Rounded)).render(false);
        let out_no =
            ArchitectureRenderer::new(&spec_no_lbl, Theme::new(BoxStyle::Rounded)).render(false);

        let lines_with: Vec<&str> = out_with.lines().collect();
        let lines_no: Vec<&str> = out_no.lines().collect();

        // (1) Output line 0 == title row of no-label render
        assert_eq!(lines_with[0], lines_no[0]);

        // Find container top border index
        let container_top_idx = lines_with
            .iter()
            .position(|l| l.contains("ColumnContainer"))
            .unwrap();

        // (2) Every label line index > container top border line index (if label rendered)
        // (3) No line above or at container top border contains label text
        for (idx, line) in lines_with.iter().enumerate() {
            if idx <= container_top_idx {
                assert!(
                    !line.contains("HTTP"),
                    "Label found at or above top border on line {idx}"
                );
            }
        }

        // Second spec: stacked containers
        let stacked_spec = ArchitectureSpec {
            style: BoxStyle::Rounded,
            title: Some("Stacked".to_string()),
            containers: vec![
                ContainerSpec {
                    id: "c1".to_string(),
                    title: "First".to_string(),
                    layout: ContainerLayout::Row,
                    color: None,
                    items: vec![ContainerItem::Leaf(LeafComponent {
                        id: "a".to_string(),
                        name: "A".to_string(),
                        properties: vec![],
                        color: None,
                    })],
                },
                ContainerSpec {
                    id: "c2".to_string(),
                    title: "Second".to_string(),
                    layout: ContainerLayout::Row,
                    color: None,
                    items: vec![ContainerItem::Leaf(LeafComponent {
                        id: "b".to_string(),
                        name: "B".to_string(),
                        properties: vec![],
                        color: None,
                    })],
                },
            ],
            connections: vec![EdgeSpec {
                from: "a".to_string(),
                to: "b".to_string(),
                label: Some("CALL".to_string()),
                arrow: ArrowDirection::Forward,
                dashed: false,
                thick: false,
                color: None,
            }],
        };

        let stacked_out =
            ArchitectureRenderer::new(&stacked_spec, Theme::new(BoxStyle::Rounded)).render(false);
        let first_top_idx = stacked_out
            .lines()
            .position(|l| l.contains("First"))
            .unwrap();
        for (idx, line) in stacked_out.lines().enumerate() {
            if idx <= first_top_idx {
                assert!(!line.contains("CALL"));
            }
        }
    }

    #[test]
    fn test_tb_label_does_not_sever_border() {
        let spec = ArchitectureSpec {
            style: BoxStyle::Rounded,
            title: None,
            containers: vec![ContainerSpec {
                id: "c".to_string(),
                title: "Box".to_string(),
                layout: ContainerLayout::Column,
                color: None,
                items: vec![
                    ContainerItem::Leaf(LeafComponent {
                        id: "alpha".to_string(),
                        name: "Alpha".to_string(),
                        properties: vec![],
                        color: None,
                    }),
                    ContainerItem::Leaf(LeafComponent {
                        id: "beta".to_string(),
                        name: "Beta".to_string(),
                        properties: vec![],
                        color: None,
                    }),
                ],
            }],
            connections: vec![EdgeSpec {
                from: "alpha".to_string(),
                to: "beta".to_string(),
                label: Some("DATA".to_string()),
                arrow: ArrowDirection::Forward,
                dashed: false,
                thick: false,
                color: None,
            }],
        };

        let out = ArchitectureRenderer::new(&spec, Theme::new(BoxStyle::Rounded)).render(false);
        let lines: Vec<&str> = out.lines().collect();

        // Alpha bottom border has full corner pair
        let alpha_bottom = lines.iter().find(|l| l.contains('╰') && l.contains('╯'));
        assert!(alpha_bottom.is_some());
        assert!(!alpha_bottom.unwrap().contains("DATA"));

        // If label is present, it is at most once and never on a border row
        let lbl_count = out.matches("DATA").count();
        assert!(lbl_count <= 1);
        if let Some(lbl_idx) = lines.iter().position(|l| l.contains("DATA")) {
            let line = lines[lbl_idx];
            assert!(!line.contains('─'));
        }
    }

    #[test]
    fn test_rtl_wrap_label_stays_inside_container() {
        let spec = ArchitectureSpec {
            style: BoxStyle::Rounded,
            title: None,
            containers: vec![ContainerSpec {
                id: "c".to_string(),
                title: "RowContainer".to_string(),
                layout: ContainerLayout::Row,
                color: None,
                items: vec![
                    ContainerItem::Leaf(LeafComponent {
                        id: "alpha".to_string(),
                        name: "Alpha".to_string(),
                        properties: vec![],
                        color: None,
                    }),
                    ContainerItem::Leaf(LeafComponent {
                        id: "beta".to_string(),
                        name: "Beta".to_string(),
                        properties: vec![],
                        color: None,
                    }),
                ],
            }],
            connections: vec![EdgeSpec {
                from: "beta".to_string(),
                to: "alpha".to_string(),
                label: Some("SYNC".to_string()),
                arrow: ArrowDirection::Forward,
                dashed: false,
                thick: false,
                color: None,
            }],
        };

        let out = ArchitectureRenderer::new(&spec, Theme::new(BoxStyle::Rounded)).render(false);
        let lines: Vec<&str> = out.lines().collect();

        // Container bottom border is the last non-empty line
        let bottom_border_idx = lines
            .iter()
            .rposition(|l| l.contains('╰') && l.contains('╯'))
            .unwrap();

        if let Some(lbl_idx) = lines.iter().position(|l| l.contains("SYNC")) {
            assert!(
                lbl_idx < bottom_border_idx,
                "Label on or below container bottom border"
            );
        }

        // Bottom border has full corner pair and is intact
        let b_line = lines[bottom_border_idx];
        assert!(b_line.contains('╰') && b_line.contains('╯'));
        assert!(!b_line.contains("SYNC"));
    }

    #[test]
    fn test_rtl_label_non_bottom_row_unchanged() {
        // Taller container where Alpha and Beta have a taller sibling Gamma
        let spec = ArchitectureSpec {
            style: BoxStyle::Rounded,
            title: None,
            containers: vec![ContainerSpec {
                id: "c".to_string(),
                title: "TallerContainer".to_string(),
                layout: ContainerLayout::Row,
                color: None,
                items: vec![
                    ContainerItem::Leaf(LeafComponent {
                        id: "alpha".to_string(),
                        name: "Alpha".to_string(),
                        properties: vec![],
                        color: None,
                    }),
                    ContainerItem::Leaf(LeafComponent {
                        id: "beta".to_string(),
                        name: "Beta".to_string(),
                        properties: vec![],
                        color: None,
                    }),
                    ContainerItem::Leaf(LeafComponent {
                        id: "gamma".to_string(),
                        name: "Gamma".to_string(),
                        properties: vec![
                            ("P1".to_string(), "V1".to_string()),
                            ("P2".to_string(), "V2".to_string()),
                            ("P3".to_string(), "V3".to_string()),
                        ],
                        color: None,
                    }),
                ],
            }],
            connections: vec![EdgeSpec {
                from: "beta".to_string(),
                to: "alpha".to_string(),
                label: Some("SYNC".to_string()),
                arrow: ArrowDirection::Forward,
                dashed: false,
                thick: false,
                color: None,
            }],
        };

        let out = ArchitectureRenderer::new(&spec, Theme::new(BoxStyle::Rounded)).render(false);
        assert!(out.contains("SYNC"));
    }

    #[test]
    fn test_skipped_connection_warnings_classification() {
        let mut bounds = HashMap::new();
        bounds.insert(
            "frontend".to_string(),
            BoxBounds {
                x: 2,
                y: 2,
                width: 10,
                height: 4,
            },
        );
        bounds.insert(
            "backend".to_string(),
            BoxBounds {
                x: 20,
                y: 2,
                width: 10,
                height: 4,
            },
        );

        let mut container_ids = HashSet::new();
        container_ids.insert("k8s".to_string());

        let connections = vec![
            // Valid
            EdgeSpec {
                from: "frontend".to_string(),
                to: "backend".to_string(),
                label: None,
                arrow: ArrowDirection::Forward,
                dashed: false,
                thick: false,
                color: None,
            },
            // Unknown id
            EdgeSpec {
                from: "frontend".to_string(),
                to: "ghost".to_string(),
                label: None,
                arrow: ArrowDirection::Forward,
                dashed: false,
                thick: false,
                color: None,
            },
            // Self-loop
            EdgeSpec {
                from: "frontend".to_string(),
                to: "frontend".to_string(),
                label: None,
                arrow: ArrowDirection::Forward,
                dashed: false,
                thick: false,
                color: None,
            },
            // Container ID
            EdgeSpec {
                from: "frontend".to_string(),
                to: "k8s".to_string(),
                label: None,
                arrow: ArrowDirection::Forward,
                dashed: false,
                thick: false,
                color: None,
            },
            // Duplicated unknown id
            EdgeSpec {
                from: "backend".to_string(),
                to: "ghost".to_string(),
                label: None,
                arrow: ArrowDirection::Forward,
                dashed: false,
                thick: false,
                color: None,
            },
        ];

        let warnings = skipped_connection_warnings(&connections, &bounds, &container_ids);
        assert_eq!(
            warnings.len(),
            3,
            "Expected exactly 3 deduped warnings, got {warnings:?}"
        );
        assert!(
            warnings
                .iter()
                .any(|w| w.contains("self-loop") && w.contains("frontend"))
        );
        assert!(
            warnings
                .iter()
                .any(|w| w.contains("container ID") && w.contains("k8s"))
        );
        assert!(
            warnings
                .iter()
                .any(|w| w.contains("unknown component ID") && w.contains("ghost"))
        );
    }
}
