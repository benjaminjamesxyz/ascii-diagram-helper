use super::*;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::*;
    use crate::theme::BoxStyle;

    #[test]
    fn test_simple_flowchart_tb() {
        let spec = FlowchartSpec {
            direction: LayoutDirection::TB,
            style: BoxStyle::Rounded,
            title: None,
            subgraphs: Vec::new(),
            nodes: vec![
                NodeSpec {
                    id: "A".to_string(),
                    label: "Client".to_string(),
                    shape: NodeShape::Box,
                    dashed_border: false,
                    fill_color: None,
                    border_level: 0,
                    color: None,
                },
                NodeSpec {
                    id: "B".to_string(),
                    label: "Server".to_string(),
                    shape: NodeShape::Box,
                    dashed_border: false,
                    fill_color: None,
                    border_level: 0,
                    color: None,
                },
            ],
            edges: vec![EdgeSpec {
                from: "A".to_string(),
                to: "B".to_string(),
                label: Some("HTTP".to_string()),
                arrow: ArrowDirection::Forward,
                dashed: false,
                thick: false,
                color: None,
            }],
        };

        let renderer = FlowchartRenderer::new(&spec, Theme::new(BoxStyle::Rounded));
        let result = renderer.render(false);
        assert!(result.contains("Client"));
        assert!(result.contains("Server"));
        assert!(result.contains("HTTP"));
        assert!(result.contains("▼"));
    }

    #[test]
    fn test_simple_flowchart_lr() {
        let spec = FlowchartSpec {
            direction: LayoutDirection::LR,
            style: BoxStyle::Ascii,
            title: None,
            subgraphs: Vec::new(),
            nodes: vec![
                NodeSpec {
                    id: "A".to_string(),
                    label: "Start".to_string(),
                    shape: NodeShape::Box,
                    dashed_border: false,
                    fill_color: None,
                    border_level: 0,
                    color: None,
                },
                NodeSpec {
                    id: "B".to_string(),
                    label: "End".to_string(),
                    shape: NodeShape::Box,
                    dashed_border: false,
                    fill_color: None,
                    border_level: 0,
                    color: None,
                },
            ],
            edges: vec![EdgeSpec {
                from: "A".to_string(),
                to: "B".to_string(),
                label: None,
                arrow: ArrowDirection::Forward,
                dashed: false,
                thick: false,
                color: None,
            }],
        };

        let renderer = FlowchartRenderer::new(&spec, Theme::ascii());
        let result = renderer.render(false);
        assert!(result.contains("Start"));
        assert!(result.contains("End"));
        assert!(result.contains('>'));
    }

    #[test]
    fn test_diamond_decision_flowchart() {
        let spec = FlowchartSpec {
            direction: LayoutDirection::TB,
            style: BoxStyle::Rounded,
            title: None,
            subgraphs: Vec::new(),
            nodes: vec![
                NodeSpec {
                    id: "A".to_string(),
                    label: "Start".to_string(),
                    shape: NodeShape::Box,
                    dashed_border: false,
                    fill_color: None,
                    border_level: 0,
                    color: None,
                },
                NodeSpec {
                    id: "B".to_string(),
                    label: "Is Valid?".to_string(),
                    shape: NodeShape::Diamond,
                    dashed_border: false,
                    fill_color: None,
                    border_level: 0,
                    color: None,
                },
                NodeSpec {
                    id: "C".to_string(),
                    label: "Proceed".to_string(),
                    shape: NodeShape::Box,
                    dashed_border: false,
                    fill_color: None,
                    border_level: 0,
                    color: None,
                },
            ],
            edges: vec![
                EdgeSpec {
                    from: "A".to_string(),
                    to: "B".to_string(),
                    label: None,
                    arrow: ArrowDirection::Forward,
                    dashed: false,
                    thick: false,
                    color: None,
                },
                EdgeSpec {
                    from: "B".to_string(),
                    to: "C".to_string(),
                    label: Some("Yes".to_string()),
                    arrow: ArrowDirection::Forward,
                    dashed: false,
                    thick: false,
                    color: None,
                },
            ],
        };

        let renderer = FlowchartRenderer::new(&spec, Theme::new(BoxStyle::Rounded));
        let result = renderer.render(false);
        assert!(result.contains("Is Valid?"));
        assert!(result.contains("◇"));
        // Decision box borders follow the theme: rounded style yields rounded
        // corners, never the double-line glyphs of the old hard-coded box
        assert!(result.contains('╭'));
        assert!(result.contains('│'));
        assert!(!result.contains('╔'));
        assert!(result.contains("Proceed"));
    }

    #[test]
    fn test_feedback_loop_ranking_and_rendering() {
        let spec = FlowchartSpec {
            direction: LayoutDirection::TB,
            style: BoxStyle::Rounded,
            title: None,
            subgraphs: Vec::new(),
            nodes: vec![
                NodeSpec {
                    id: "Ref".to_string(),
                    label: "Target".to_string(),
                    shape: NodeShape::Box,
                    dashed_border: false,
                    fill_color: None,
                    border_level: 0,
                    color: None,
                },
                NodeSpec {
                    id: "Sum".to_string(),
                    label: "Error".to_string(),
                    shape: NodeShape::Circle,
                    dashed_border: false,
                    fill_color: None,
                    border_level: 0,
                    color: None,
                },
                NodeSpec {
                    id: "PID".to_string(),
                    label: "Controller".to_string(),
                    shape: NodeShape::Box,
                    dashed_border: false,
                    fill_color: None,
                    border_level: 0,
                    color: None,
                },
                NodeSpec {
                    id: "Plant".to_string(),
                    label: "Motor".to_string(),
                    shape: NodeShape::Box,
                    dashed_border: false,
                    fill_color: None,
                    border_level: 0,
                    color: None,
                },
            ],
            edges: vec![
                EdgeSpec {
                    from: "Ref".to_string(),
                    to: "Sum".to_string(),
                    label: None,
                    arrow: ArrowDirection::Forward,
                    dashed: false,
                    thick: false,
                    color: None,
                },
                EdgeSpec {
                    from: "Sum".to_string(),
                    to: "PID".to_string(),
                    label: None,
                    arrow: ArrowDirection::Forward,
                    dashed: false,
                    thick: false,
                    color: None,
                },
                EdgeSpec {
                    from: "PID".to_string(),
                    to: "Plant".to_string(),
                    label: None,
                    arrow: ArrowDirection::Forward,
                    dashed: false,
                    thick: false,
                    color: None,
                },
                EdgeSpec {
                    from: "Plant".to_string(),
                    to: "Sum".to_string(),
                    label: Some("Feedback".to_string()),
                    arrow: ArrowDirection::Forward,
                    dashed: false,
                    thick: false,
                    color: None,
                },
            ],
        };

        let renderer = FlowchartRenderer::new(&spec, Theme::new(BoxStyle::Rounded));
        let mut nodes = renderer.prepare_nodes(&Blocks::empty());
        let idx = renderer.index_of();
        let layers = renderer.assign_ranks(&mut nodes, &idx);

        // Verify ranks are ordered: Ref (0) -> Sum (1) -> PID (2) -> Plant (3)
        assert_eq!(nodes[idx["Ref"]].rank, 0);
        assert_eq!(nodes[idx["Sum"]].rank, 1);
        assert_eq!(nodes[idx["PID"]].rank, 2);
        assert_eq!(nodes[idx["Plant"]].rank, 3);
        assert_eq!(layers.len(), 4);

        let result = renderer.render(false);
        assert!(result.contains("Target"));
        assert!(result.contains("Error"));
        assert!(result.contains("Controller"));
        assert!(result.contains("Motor"));
        assert!(result.contains("Feedback"));
    }
}

#[cfg(test)]
mod self_loop_tests {
    use super::*;
    use crate::schema::*;
    use crate::theme::Theme;

    fn spec(direction: LayoutDirection) -> FlowchartSpec {
        FlowchartSpec {
            direction,
            style: BoxStyle::Rounded,
            title: None,
            subgraphs: Vec::new(),
            nodes: vec![NodeSpec {
                id: "A".to_string(),
                label: "Box".to_string(),
                shape: NodeShape::Box,
                dashed_border: false,
                fill_color: None,
                border_level: 0,
                color: None,
            }],
            edges: vec![EdgeSpec {
                from: "A".to_string(),
                to: "A".to_string(),
                label: Some("retry".to_string()),
                arrow: ArrowDirection::Forward,
                dashed: false,
                thick: false,
                color: None,
            }],
        }
    }

    #[test]
    fn test_self_loop_tb() {
        let spec = spec(LayoutDirection::TB);
        let out = FlowchartRenderer::new(&spec, Theme::new(BoxStyle::Rounded)).render(false);
        assert!(out.contains("Box"));
        assert!(out.contains("retry"), "self-loop label must render");
        assert!(out.contains('◄'), "re-entry arrowhead must render");
    }

    #[test]
    fn test_self_loop_lr() {
        let spec = spec(LayoutDirection::LR);
        let out = FlowchartRenderer::new(&spec, Theme::new(BoxStyle::Rounded)).render(false);
        assert!(out.contains("Box"));
        assert!(out.contains("retry"));
        assert!(out.contains('◄'));
    }

    /// Regression: `A --> A` at x=0 used to panic with `attempt to subtract
    /// with overflow` in the same-rank branch.
    #[test]
    fn test_self_loop_at_origin_no_panic() {
        let spec = spec(LayoutDirection::TB);
        let out = FlowchartRenderer::new(&spec, Theme::new(BoxStyle::Rounded)).render(false);
        assert!(out.contains("Box"));
    }
}

#[cfg(test)]
mod jump_group_tests {
    use super::*;
    use crate::schema::*;
    use crate::theme::BoxStyle;

    #[test]
    fn test_multi_jump_shared_track() {
        let spec = FlowchartSpec {
            style: BoxStyle::Rounded,
            title: None,
            subgraphs: Vec::new(),
            direction: LayoutDirection::TB,
            nodes: vec![
                NodeSpec {
                    id: "W".into(),
                    label: "Watchdog".into(),
                    shape: NodeShape::Box,
                    dashed_border: false,
                    fill_color: None,
                    border_level: 0,
                    color: None,
                },
                NodeSpec {
                    id: "A".into(),
                    label: "Task A".into(),
                    shape: NodeShape::Box,
                    dashed_border: false,
                    fill_color: None,
                    border_level: 0,
                    color: None,
                },
                NodeSpec {
                    id: "B".into(),
                    label: "Task B".into(),
                    shape: NodeShape::Box,
                    dashed_border: false,
                    fill_color: None,
                    border_level: 0,
                    color: None,
                },
                NodeSpec {
                    id: "C".into(),
                    label: "Task C".into(),
                    shape: NodeShape::Box,
                    dashed_border: false,
                    fill_color: None,
                    border_level: 0,
                    color: None,
                },
                NodeSpec {
                    id: "Z".into(),
                    label: "Done".into(),
                    shape: NodeShape::Box,
                    dashed_border: false,
                    fill_color: None,
                    border_level: 0,
                    color: None,
                },
            ],
            edges: vec![
                EdgeSpec {
                    from: "W".into(),
                    to: "A".into(),
                    label: None,
                    arrow: ArrowDirection::Forward,
                    dashed: true,
                    thick: false,
                    color: None,
                },
                EdgeSpec {
                    from: "W".into(),
                    to: "B".into(),
                    label: None,
                    arrow: ArrowDirection::Forward,
                    dashed: true,
                    thick: false,
                    color: None,
                },
                EdgeSpec {
                    from: "W".into(),
                    to: "C".into(),
                    label: None,
                    arrow: ArrowDirection::Forward,
                    dashed: true,
                    thick: false,
                    color: None,
                },
                EdgeSpec {
                    from: "A".into(),
                    to: "Z".into(),
                    label: None,
                    arrow: ArrowDirection::Forward,
                    dashed: false,
                    thick: false,
                    color: None,
                },
                EdgeSpec {
                    from: "B".into(),
                    to: "Z".into(),
                    label: None,
                    arrow: ArrowDirection::Forward,
                    dashed: false,
                    thick: false,
                    color: None,
                },
                EdgeSpec {
                    from: "C".into(),
                    to: "Z".into(),
                    label: None,
                    arrow: ArrowDirection::Forward,
                    dashed: false,
                    thick: false,
                    color: None,
                },
            ],
        };
        let r = FlowchartRenderer::new(&spec, crate::theme::Theme::new(BoxStyle::Rounded));
        let out = r.render(false);
        // Every target must have an arrowhead directly above its box
        let lines: Vec<&str> = out.lines().collect();
        let arrow_row = lines.iter().find(|l| l.contains('▼')).expect("arrow row");
        let count = arrow_row.matches('▼').count();
        assert_eq!(count, 3, "expected 3 drops, row: {arrow_row}");
    }

    #[test]
    fn test_subgraph_group_boxes_rendered() {
        let dsl = "graph TB
            subgraph front [Front End]
              A[Web] --> B[API]
            end
            subgraph back [Back End]
              C[Auth]
            end
            B --> C";
        let spec = crate::parse_dsl_or_json(dsl, BoxStyle::Rounded).unwrap();
        let DiagramSpec::Flowchart(f) = spec else {
            panic!("Expected flowchart")
        };
        let out = FlowchartRenderer::new(&f, Theme::new(BoxStyle::Rounded)).render(false);
        assert!(out.contains("Front End"), "group title on border: {out}");
        assert!(out.contains("Back End"), "second group title: {out}");
        // Group borders enclose member nodes: the Web box must sit right of a
        // group side border
        let web_row = out
            .lines()
            .find(|l| l.contains("│ Web │"))
            .expect("web row");
        assert!(
            web_row.trim_start().starts_with('│'),
            "group border left of member: {web_row}"
        );
    }

    #[test]
    fn test_thick_edge_renders_heavy_glyphs() {
        let spec = FlowchartSpec {
            direction: LayoutDirection::TB,
            style: BoxStyle::Rounded,
            title: None,
            subgraphs: Vec::new(),
            nodes: vec![
                NodeSpec {
                    id: "A".into(),
                    label: "Start".into(),
                    shape: NodeShape::Box,
                    dashed_border: false,
                    fill_color: None,
                    border_level: 0,
                    color: None,
                },
                NodeSpec {
                    id: "B".into(),
                    label: "End".into(),
                    shape: NodeShape::Box,
                    dashed_border: false,
                    fill_color: None,
                    border_level: 0,
                    color: None,
                },
            ],
            edges: vec![EdgeSpec {
                from: "A".into(),
                to: "B".into(),
                label: None,
                arrow: ArrowDirection::Forward,
                dashed: false,
                thick: true,
                color: None,
            }],
        };
        let out = FlowchartRenderer::new(&spec, Theme::new(BoxStyle::Rounded)).render(false);
        assert!(out.contains('┃'), "thick vertical run: {out}");
        // The edge drop itself must be heavy; node borders are sharp by design
        let drop_lines: Vec<&str> = out
            .lines()
            .filter(|l| l.trim().chars().all(|c| c == '┃'))
            .collect();
        assert!(
            !drop_lines.is_empty(),
            "pure heavy drop line expected: {out}"
        );
    }

    #[test]
    fn test_thick_edge_ascii_style() {
        let spec = FlowchartSpec {
            direction: LayoutDirection::LR,
            style: BoxStyle::Ascii,
            title: None,
            subgraphs: Vec::new(),
            nodes: vec![
                NodeSpec {
                    id: "A".into(),
                    label: "Start".into(),
                    shape: NodeShape::Box,
                    dashed_border: false,
                    fill_color: None,
                    border_level: 0,
                    color: None,
                },
                NodeSpec {
                    id: "B".into(),
                    label: "End".into(),
                    shape: NodeShape::Box,
                    dashed_border: false,
                    fill_color: None,
                    border_level: 0,
                    color: None,
                },
            ],
            edges: vec![EdgeSpec {
                from: "A".into(),
                to: "B".into(),
                label: None,
                arrow: ArrowDirection::Forward,
                dashed: false,
                thick: true,
                color: None,
            }],
        };
        let out = FlowchartRenderer::new(&spec, Theme::ascii()).render(false);
        assert!(out.contains('='), "ascii thick horizontal run: {out}");
    }
}

#[cfg(test)]
mod subgraph_direction_tests {
    use super::*;
    use crate::schema::*;
    use crate::theme::BoxStyle;

    fn render(dsl: &str) -> String {
        match crate::parser::parse_dsl_or_json(dsl, BoxStyle::Rounded).unwrap() {
            DiagramSpec::Flowchart(f) => {
                let theme = crate::theme::Theme::new(f.style);
                FlowchartRenderer::new(&f, theme).render(false)
            }
            _ => panic!("Expected flowchart"),
        }
    }

    #[test]
    fn isolated_subgraph_direction_lr_applies() {
        let dsl = "graph TB
            A --> B
            subgraph cluster [Cluster]
              direction LR
              X --> Y
            end";
        let out = render(dsl);
        // X and Y laid out horizontally inside the block: they share a row
        let x_col = out
            .lines()
            .position(|l| l.contains("X"))
            .expect("X rendered");
        let y_col = out
            .lines()
            .position(|l| l.contains("Y"))
            .expect("Y rendered");
        assert_eq!(x_col, y_col, "X and Y on the same row (LR inside subgraph)");
        // Cluster group box wraps the pasted block
        assert!(out.lines().any(|l| l.contains("Cluster")));
        // A --> B unaffected (vertical)
        assert!(out.lines().any(|l| l.contains("A")));
        assert!(out.lines().any(|l| l.contains("B")));
    }

    #[test]
    fn mixed_subgraph_direction_falls_back_to_global() {
        // C inside the subgraph has an edge crossing the border → not
        // isolated → global TB wins (Mermaid parity)
        let dsl = "graph TB
            A --> B
            subgraph g
              direction LR
              C --> B
            end";
        let out = render(dsl);
        let c_col = out
            .lines()
            .position(|l| l.contains("C"))
            .expect("C rendered");
        let b_col = out
            .lines()
            .position(|l| l.contains("B"))
            .expect("B rendered");
        assert_ne!(
            c_col, b_col,
            "C and B on different rows (global TB still applies)"
        );
    }

    #[test]
    fn whole_diagram_is_isolated_cluster() {
        let dsl = "graph TB
            subgraph g
              direction LR
              X --> Y
            end";
        let out = render(dsl);
        assert!(out.lines().any(|l| l.contains("X")));
        assert!(out.lines().any(|l| l.contains("Y")));
    }

    #[test]
    fn isolated_subgraph_direction_tb_in_global_lr() {
        // Regression: member boxes were drawn at phantom positions over real
        // nodes because the LR node-draw loop lacked the member skip
        let dsl = "graph LR
            A --> B
            subgraph cluster
              direction TB
              X --> Y
            end";
        let out = render(dsl);
        let a_row = out
            .lines()
            .position(|l| l.contains("A"))
            .expect("A rendered");
        let b_row = out
            .lines()
            .position(|l| l.contains("B"))
            .expect("B rendered");
        assert_eq!(a_row, b_row, "A and B on the same row (global LR)");
        let x_row = out
            .lines()
            .position(|l| l.contains("X"))
            .expect("X rendered");
        let y_row = out
            .lines()
            .position(|l| l.contains("Y"))
            .expect("Y rendered");
        assert_ne!(x_row, y_row, "X above Y inside the block (TB subgraph)");
        assert!(out.lines().any(|l| l.contains("cluster")));
    }
}

#[cfg(test)]
mod fill_thick_tests {
    use super::*;
    use crate::schema::*;
    use crate::theme::BoxStyle;

    #[test]
    fn thick_border_renders_heavy_glyphs() {
        let spec = FlowchartSpec {
            direction: LayoutDirection::TB,
            style: BoxStyle::Sharp,
            title: None,
            subgraphs: Vec::new(),
            nodes: vec![NodeSpec {
                id: "A".to_string(),
                label: "Big".to_string(),
                shape: NodeShape::Box,
                dashed_border: false,
                color: None,
                fill_color: None,
                border_level: 1,
            }],
            edges: Vec::new(),
        };
        let theme = crate::theme::Theme::new(BoxStyle::Sharp);
        let out = FlowchartRenderer::new(&spec, theme).render(false);
        assert!(out.contains('┏'), "top-left heavy corner in {out}");
        assert!(out.contains('┗'), "bottom-left heavy corner in {out}");
        assert!(!out.contains('┌'), "no thin corner expected in {out}");
    }

    #[test]
    fn fill_color_colorizes_label_when_colored() {
        let spec = FlowchartSpec {
            direction: LayoutDirection::TB,
            style: BoxStyle::Sharp,
            title: None,
            subgraphs: Vec::new(),
            nodes: vec![NodeSpec {
                id: "A".to_string(),
                label: "Hot".to_string(),
                shape: NodeShape::Box,
                dashed_border: false,
                color: None,
                fill_color: Some(crate::color::Color::Red),
                border_level: 0,
            }],
            edges: Vec::new(),
        };
        let theme = crate::theme::Theme::new(BoxStyle::Sharp);
        let out = FlowchartRenderer::new(&spec, theme.clone()).render(true);
        assert!(
            out.contains("\u{1b}[31mHot"),
            "red SGR around label in {out:?}"
        );
        // Uncolored render has no SGR
        let plain = FlowchartRenderer::new(&spec, theme).render(false);
        assert!(!plain.contains('\u{1b}'), "no SGR when colored=false");
    }
}

#[cfg(test)]
mod dense_feed_tests {
    use super::*;
    use crate::schema::*;
    use crate::theme::BoxStyle;

    fn render(dsl: &str) -> String {
        match crate::parser::parse_dsl_or_json(dsl, BoxStyle::Rounded).unwrap() {
            DiagramSpec::Flowchart(f) => {
                let theme = crate::theme::Theme::new(f.style);
                FlowchartRenderer::new(&f, theme).render(false)
            }
            _ => panic!("Expected flowchart"),
        }
    }

    #[test]
    fn no_adjacent_double_arrowheads_on_shared_targets() {
        // Regression: a target fed by one aligned edge (drawn at the source
        // column) and one bend edge (drawn at the target center) used to get
        // adjacent ▼▼ arrowheads. All incoming edges now converge on one
        // arrowhead column per target.
        let dsl = "graph TB
            MAIN --> SCHED
            MAIN --> COMM
            WDG -.-> SCHED
            WDG -.-> COMM";
        let out = render(dsl);
        assert!(
            !out.contains("▼▼"),
            "no adjacent double arrowheads expected:\n{out}"
        );
    }

    #[test]
    fn dense_supervisory_feeds_single_arrowheads() {
        let dsl = "graph TB
            MAIN --> SCHED
            MAIN --> COMM
            SCHED --> T1
            SCHED --> T2
            COMM --> T3
            COMM --> T4
            T1 --> ACT
            T2 --> SENSE
            WDG -.-> SCHED
            WDG -.-> COMM
            WDG -.-> T2
            WDG -.-> T3
            WDG -.-> ACT";
        let out = render(dsl);
        assert!(
            !out.contains("▼▼"),
            "dense feeds must not double arrowheads:\n{out}"
        );
        // All five targets still receive their edges
        for id in ["SCHED", "COMM", "T1", "T2", "T3", "T4", "ACT", "SENSE"] {
            assert!(out.contains(id), "{id} missing");
        }
    }
}

#[cfg(test)]
mod border_level_tests {
    use super::*;
    use crate::schema::*;
    use crate::theme::BoxStyle;

    fn render_with_level(level: u8) -> String {
        let spec = FlowchartSpec {
            direction: LayoutDirection::TB,
            style: BoxStyle::Sharp,
            title: None,
            subgraphs: Vec::new(),
            nodes: vec![NodeSpec {
                id: "A".to_string(),
                label: "X".to_string(),
                shape: NodeShape::Box,
                dashed_border: false,
                color: None,
                fill_color: None,
                border_level: level,
            }],
            edges: Vec::new(),
        };
        let theme = crate::theme::Theme::new(BoxStyle::Sharp);
        FlowchartRenderer::new(&spec, theme).render(false)
    }

    #[test]
    fn level_0_renders_sharp() {
        let out = render_with_level(0);
        assert!(out.contains('┌') && !out.contains('┏') && !out.contains('╔'));
    }

    #[test]
    fn level_1_renders_heavy() {
        let out = render_with_level(1);
        assert!(out.contains('┏') && out.contains('┗') && !out.contains('╔'));
    }

    #[test]
    fn level_2_renders_double() {
        let out = render_with_level(2);
        assert!(out.contains('╔') && out.contains('╚') && !out.contains('┏'));
    }
}

#[cfg(test)]
mod crossing_junction_tests {
    use super::*;
    use crate::schema::*;
    use crate::theme::BoxStyle;

    #[test]
    fn dash_run_crossing_solid_line_renders_junction() {
        // Regression: dashed horizontal runs stamped ╌ over solid vertical
        // lines, losing the vertical stroke at the crossing. Crossings now
        // merge into junction glyphs (┼) with both strokes continuous.
        let dsl = "graph TB
            A --> B
            A --> C
            B --> D
            C --> D
            WDG -.-> D";
        match crate::parser::parse_dsl_or_json(dsl, BoxStyle::Rounded).unwrap() {
            DiagramSpec::Flowchart(f) => {
                let theme = crate::theme::Theme::new(f.style);
                let out = FlowchartRenderer::new(&f, theme).render(false);
                assert!(out.contains('┼'), "crossing junction expected:\n{out}");
            }
            _ => panic!("Expected flowchart"),
        }
    }
}

#[cfg(test)]
mod track_side_tests {
    use super::*;
    use crate::schema::*;
    use crate::theme::BoxStyle;

    #[test]
    fn left_heavy_feeds_route_left_corridor() {
        // Majority of jump targets left of the source → shared track on the
        // left corridor (shorter runs, fewer band crossings)
        let dsl = "graph TB
            CORE --> A1
            CORE --> A2
            A1 --> B1
            A2 --> B2
            WDG -.-> A1
            WDG -.-> B1
            WDG -.-> B2";
        match crate::parser::parse_dsl_or_json(dsl, BoxStyle::Rounded).unwrap() {
            DiagramSpec::Flowchart(f) => {
                let theme = crate::theme::Theme::new(f.style);
                let out = FlowchartRenderer::new(&f, theme).render(false);
                // Left track column: a dash glyph in the first columns of the
                // mid rows (the track runs the diagram height)
                let left_track = out
                    .lines()
                    .filter(|l| l.starts_with('┆') || l.starts_with('|'))
                    .count();
                assert!(left_track >= 3, "left corridor track expected:\n{out}");
                // All targets still fed
                for id in ["A1", "B1", "B2"] {
                    assert!(out.contains(id));
                }
            }
            _ => panic!("Expected flowchart"),
        }
    }
}

#[cfg(test)]
mod nested_direction_tests {
    use super::*;
    use crate::schema::*;
    use crate::theme::BoxStyle;

    #[test]
    fn isolated_child_of_non_isolated_parent_keeps_direction() {
        // `inner` has only internal edges → moves to its own LR block even
        // though its parent `outer` has external edges. The parent's group
        // box must NOT inflate to the moved child's phantom coordinates.
        let dsl = "graph TB
            MAIN --> A
            A --> B
            subgraph outer
              subgraph inner [Inner LR]
                direction LR
                X --> Y
              end
              MAIN --> Z
            end";
        match crate::parser::parse_dsl_or_json(dsl, BoxStyle::Rounded).unwrap() {
            DiagramSpec::Flowchart(f) => {
                let theme = crate::theme::Theme::new(f.style);
                let out = FlowchartRenderer::new(&f, theme).render(false);
                // Inner block rendered in its own orientation (X left of Y)
                let x_row = out.lines().position(|l| l.contains("X")).unwrap();
                let y_row = out.lines().position(|l| l.contains("Y")).unwrap();
                assert_eq!(x_row, y_row, "inner cluster laid out LR:\n{out}");
                // Parent box does not reach column 0 via phantom coords:
                // MAIN/A/B render outside the outer box's top-left
                let outer_top = out
                    .lines()
                    .position(|l| l.contains("╭─ outer"))
                    .expect("outer box");
                let line = &out.lines().nth(outer_top).unwrap();
                let box_x = line.find('╭').unwrap();
                assert!(
                    box_x > 0,
                    "outer box inflated by moved child's phantom coords:\n{out}"
                );
            }
            _ => panic!("Expected flowchart"),
        }
    }
}

#[cfg(test)]
mod dashed_crossing_tests {
    use super::*;
    use crate::schema::*;
    use crate::theme::BoxStyle;

    #[test]
    fn dashed_x_dashed_crossing_renders_junction() {
        // Two dashed runs crossing: the crossing cell becomes a solid cross
        // so neither stroke loses continuity (was: one dash overwrote the
        // other, leaving a gap in one run)
        let dsl = "graph TB
            T --> M
            S -.-> M
            S -.-> R
            T -.-> R";
        match crate::parser::parse_dsl_or_json(dsl, BoxStyle::Rounded).unwrap() {
            DiagramSpec::Flowchart(f) => {
                let theme = crate::theme::Theme::new(f.style);
                let out = FlowchartRenderer::new(&f, theme).render(false);
                assert!(
                    out.matches('┼').count() >= 4,
                    "dash×dash junctions expected:\n{out}"
                );
            }
            _ => panic!("Expected flowchart"),
        }
    }
}

#[cfg(test)]
mod barycenter_tests {
    use super::*;
    use crate::schema::*;
    use crate::theme::BoxStyle;

    #[test]
    fn reduce_crossings_fixes_crossed_layer_order() {
        // 6 nodes, ranks [0,0,1,1,2,2]; edges A→C, B→D, A→D, C→F, D→E, C→E.
        // Layers given with L2 = [E, F] (E left of F) which crosses C→F
        // against D→E. Barycenter sweeps must settle L2 = [F, E].
        let spec = FlowchartSpec {
            direction: LayoutDirection::TB,
            style: BoxStyle::Sharp,
            title: None,
            subgraphs: Vec::new(),
            nodes: ["A", "B", "C", "D", "E", "F"]
                .iter()
                .map(|id| NodeSpec {
                    id: id.to_string(),
                    label: id.to_string(),
                    shape: NodeShape::Box,
                    dashed_border: false,
                    color: None,
                    fill_color: None,
                    border_level: 0,
                })
                .collect(),
            edges: vec![
                ("A", "C"),
                ("B", "D"),
                ("A", "D"),
                ("C", "F"),
                ("D", "E"),
                ("C", "E"),
            ]
            .into_iter()
            .map(|(f, t)| EdgeSpec {
                from: f.to_string(),
                to: t.to_string(),
                label: None,
                arrow: ArrowDirection::Forward,
                dashed: false,
                thick: false,
                color: None,
            })
            .collect(),
        };
        let renderer = FlowchartRenderer::new(&spec, crate::theme::Theme::new(BoxStyle::Sharp));
        let idx = renderer.index_of();
        let mut layers: Vec<Vec<usize>> = vec![
            vec![idx["A"], idx["B"]],
            vec![idx["C"], idx["D"]],
            vec![idx["E"], idx["F"]],
        ];
        renderer.reduce_crossings(&mut layers, &idx);
        let l2: Vec<&str> = layers[2]
            .iter()
            .map(|&i| spec.nodes[i].id.as_str())
            .collect();
        assert_eq!(l2, vec!["F", "E"], "L2 reordered to kill the crossing");
    }
}

#[cfg(test)]
mod supernode_tests {
    use super::*;
    use crate::schema::*;
    use crate::theme::BoxStyle;

    #[test]
    fn non_isolated_cluster_renders_as_supernode_block() {
        // Cluster with external edges + own direction: members collapse into
        // a phantom node; the block is pasted at the phantom's position with
        // its own orientation (LR inside a TB graph)
        let dsl = "graph TB
            SENSORS --> FILTER
            FILTER --> CTRL
            CTRL --> OUT
            subgraph comms [Comms LR]
              direction LR
              TX --> ENC
              ENC --> MOD
            end
            CTRL --> TX
            MOD --> OUT";
        match crate::parser::parse_dsl_or_json(dsl, BoxStyle::Rounded).unwrap() {
            DiagramSpec::Flowchart(f) => {
                let theme = crate::theme::Theme::new(f.style);
                let out = FlowchartRenderer::new(&f, theme).render(false);
                // Cluster content horizontal
                let tx = out.lines().position(|l| l.contains("TX")).unwrap();
                let enc = out.lines().position(|l| l.contains("ENC")).unwrap();
                assert_eq!(tx, enc, "cluster laid out LR inside TB graph:\n{out}");
                // Whole chain present
                for id in ["SENSORS", "FILTER", "CTRL", "OUT", "MOD"] {
                    assert!(out.contains(id), "{id} missing");
                }
                // Group box present
                assert!(out.lines().any(|l| l.contains("╭─ Comms LR")));
            }
            _ => panic!("Expected flowchart"),
        }
    }

    // ---- Regression: theme-aware decision boxes + mixed-weight edge routing ----

    const REPRO_DSL: &str = "graph TD; S([Start]) --> C{Valid?}; C -->|yes| P[Process]; C -->|no| E[Error]; P -.->|retry| S; P ==> D((Done))";

    /// Dashed edge-stroke glyphs (`-.->` runs).
    const DASH_GLYPHS: [char; 6] = ['╌', '┆', '┄', '┊', '╍', '╏'];

    /// Heavy edge-stroke glyphs (`==>` runs).
    const HEAVY_GLYPHS: [char; 2] = ['━', '┃'];

    /// Light / double node-box border chars. Deliberately excludes `┼ ╬ ╋
    /// ─ ═ ╭╮╰╯`-family crosses shared with junction resolution, which
    /// dashed crossings render as.
    const BOX_BORDER_GLYPHS: [char; 18] = [
        '│', '║', '┌', '┐', '└', '┘', '├', '┤', '┬', '┴', '╔', '╗', '╚', '╝', '╠', '╣', '╦', '╩',
    ];

    /// Heavy node-box border chars (heavy theme / weighted boxes).
    const HEAVY_BORDER_GLYPHS: [char; 8] = ['┏', '┓', '┗', '┛', '┣', '┫', '┳', '┻'];

    fn render_flow_dsl(dsl: &str, style: BoxStyle) -> String {
        match crate::parser::parse_dsl_or_json(dsl, style).unwrap() {
            DiagramSpec::Flowchart(f) => {
                FlowchartRenderer::new(&f, Theme::new(f.style)).render(false)
            }
            _ => panic!("Expected flowchart"),
        }
    }

    /// `│ Process │┆` / `┌──────┐┆` hug pattern: a dashed/heavy stroke must
    /// never sit immediately beside a node-box border char (side-adjacency).
    /// Perpendicular feeds below/above a border stay legal, and a heavy
    /// stroke meeting its own heavy corner (`┗━━┓`) is one continuous line.
    fn assert_no_border_side_hug(output: &str) {
        let lines: Vec<Vec<char>> = output.lines().map(|l| l.chars().collect()).collect();
        for (r, line) in lines.iter().enumerate() {
            for (c, &ch) in line.iter().enumerate() {
                let (forbidden, extra): (&[char], &[char]) = if DASH_GLYPHS.contains(&ch) {
                    (&BOX_BORDER_GLYPHS, &HEAVY_BORDER_GLYPHS)
                } else if HEAVY_GLYPHS.contains(&ch) {
                    (&BOX_BORDER_GLYPHS, &[])
                } else {
                    continue;
                };
                for nc in [c.checked_sub(1), Some(c + 1)] {
                    let Some(nc) = nc else { continue };
                    if let Some(&nch) = lines[r].get(nc)
                        && (forbidden.contains(&nch) || extra.contains(&nch))
                    {
                        panic!(
                            "edge stroke '{ch}' hugs border '{nch}' at row {r} col {c}:\n{output}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn test_sharp_style_decision_node_has_no_double_glyphs() {
        let out = render_flow_dsl(REPRO_DSL, BoxStyle::Sharp);
        // Decision node (and every other node) follows the sharp theme:
        // no double-line corners, rows, or columns anywhere
        for ch in ['╔', '╗', '╚', '╝', '═', '║'] {
            assert!(!out.contains(ch), "sharp style leaked '{ch}':\n{out}");
        }
        // Decision badge kept, sharp corners used, bottom tee present
        assert!(out.contains('◇'), "decision badge missing:\n{out}");
        assert!(out.contains('┌') && out.contains('┐'));
        assert!(out.contains('┬'), "sharp bottom connector missing:\n{out}");
    }

    #[test]
    fn test_decision_box_follows_theme() {
        const DSL: &str = "graph TD; A{Pick?} --> B[Ok]";
        let cases = [
            (BoxStyle::Rounded, '╭', '╔'),
            (BoxStyle::Sharp, '┌', '╔'),
            (BoxStyle::Double, '╔', '\u{0}'),
            (BoxStyle::Heavy, '┏', '╔'),
        ];
        for (style, corner, forbidden) in cases {
            let out = render_flow_dsl(DSL, style);
            assert!(
                out.contains(corner),
                "{style:?}: decision box missing corner '{corner}':\n{out}"
            );
            if forbidden != '\u{0}' {
                assert!(
                    !out.contains(forbidden),
                    "{style:?}: decision box leaked '{forbidden}':\n{out}"
                );
            }
        }
    }

    #[test]
    fn test_mixed_edge_weights_no_border_overlap() {
        let out = render_flow_dsl(REPRO_DSL, BoxStyle::Sharp);
        // All three weights actually drew (guard against vacuous passes)
        assert!(
            out.contains('╌') && out.contains('┆'),
            "dashed edge missing:\n{out}"
        );
        assert!(
            out.contains('━') && out.contains('┃'),
            "thick edge missing:\n{out}"
        );
        assert!(out.contains("retry"), "dashed label missing:\n{out}");
        // Node borders intact
        assert!(out.contains("│ Process │"), "source box damaged:\n{out}");
        assert!(out.contains("│  Start  │"), "target box damaged:\n{out}");
        // No dashed/heavy stroke beside or through a node border; the
        // loop-back channel stays clear of every bounding box
        assert_no_border_side_hug(&out);
    }
}

#[cfg(test)]
mod database_cylinder_tests {
    use super::*;
    use crate::schema::*;
    use crate::theme::BoxStyle;

    fn render_database(style: BoxStyle) -> String {
        let dsl = "graph TD; A[App] --> DB[(PostgreSQL)]";
        match crate::parser::parse_dsl_or_json(dsl, style).unwrap() {
            DiagramSpec::Flowchart(f) => {
                FlowchartRenderer::new(&f, crate::theme::Theme::new(f.style)).render(false)
            }
            _ => panic!("Expected flowchart"),
        }
    }

    #[test]
    fn cylinder_has_curved_caps_and_paren_walls() {
        let out = render_database(BoxStyle::Rounded);
        let db_rows: Vec<&str> = out
            .lines()
            .filter(|l| {
                let t = l.trim_start();
                t.starts_with('╭') || t.starts_with('╰') || t.starts_with('(')
            })
            .collect();
        assert!(
            db_rows.len() >= 5,
            "expected cap/wall/label/wall/cap rows, got:\n{out}"
        );
        // Curved top and bottom arcs (not flat box corners)
        assert!(
            out.lines().any(|l| l.contains('╭') && l.contains('╮')),
            "missing top arc:\n{out}"
        );
        assert!(
            out.lines().any(|l| l.contains('╰') && l.contains('╯')),
            "missing bottom arc:\n{out}"
        );
        // Label row flanked by paren side walls
        let label_row = out
            .lines()
            .find(|l| l.contains("PostgreSQL"))
            .expect("label row");
        assert!(
            label_row.trim_end().ends_with(')') && label_row.trim_start().starts_with('('),
            "label row must read `( … )`, got: {label_row:?}"
        );
        // Wall row above the label is also parenthesized
        assert!(
            out.lines()
                .any(|l| l.trim_start().starts_with("(") && l.trim_end().ends_with(")")),
            "missing paren wall row:\n{out}"
        );
    }

    #[test]
    fn cylinder_has_no_flat_mid_divider() {
        let out = render_database(BoxStyle::Rounded);
        assert!(
            !out.contains('├') && !out.contains('┤'),
            "cylinder must not show a banded divider:\n{out}"
        );
    }

    #[test]
    fn cylinder_label_is_centered() {
        let out = render_database(BoxStyle::Rounded);
        let label_row = out
            .lines()
            .find(|l| l.contains("PostgreSQL"))
            .expect("label row");
        let trimmed = label_row.trim_end();
        let open = trimmed.find('(').unwrap();
        let close = trimmed.rfind(')').unwrap();
        let text_start = trimmed.find("PostgreSQL").unwrap();
        let text_end = text_start + "PostgreSQL".len();
        // Padding measured inside the paren walls must be symmetric
        assert_eq!(
            text_start - open - 1,
            close - text_end,
            "label padding must be symmetric, got: {label_row:?}"
        );
        assert!(
            text_start - open - 1 >= 1,
            "label must clear the paren walls, got: {label_row:?}"
        );
    }

    #[test]
    fn cylinder_ascii_theme_keeps_paren_walls() {
        let out = render_database(BoxStyle::Ascii);
        let label_row = out
            .lines()
            .find(|l| l.contains("PostgreSQL"))
            .expect("label row");
        assert!(
            label_row.trim_start().starts_with('(') && label_row.trim_end().ends_with(')'),
            "ascii cylinder must keep paren walls, got: {label_row:?}"
        );
        // All-ASCII cap row
        assert!(
            out.lines().any(|l| l.starts_with('+') && l.contains('-')),
            "missing ascii cap row:\n{out}"
        );
    }
}

#[cfg(test)]
mod band_gap_tests {
    use super::*;
    use crate::schema::*;
    use crate::theme::BoxStyle;

    fn render(dsl: &str) -> String {
        match crate::parser::parse_dsl_or_json(dsl, BoxStyle::Rounded).unwrap() {
            DiagramSpec::Flowchart(f) => {
                let theme = crate::theme::Theme::new(f.style);
                FlowchartRenderer::new(&f, theme).render(false)
            }
            _ => panic!("Expected flowchart"),
        }
    }

    /// Rows strictly between the bottom border of `from`'s box and the top
    /// border of `to`'s box (first occurrence of each).
    fn gap_rows<'a>(out: &'a str, from: &str, to: &str) -> Vec<&'a str> {
        let lines: Vec<&str> = out.lines().collect();
        let bottom = lines
            .iter()
            .position(|l| l.contains(&format!("│ {from}")))
            .expect(from)
            + 1; // bottom border row
        let top = lines[bottom + 1..]
            .iter()
            .position(|l| l.contains(&format!("│ {to}")))
            .expect(to)
            + bottom
            + 1; // target label row
        lines[bottom + 1..top - 1].to_vec()
    }

    #[test]
    fn unlabeled_edge_compacts_to_connector_plus_arrow() {
        // Regression: unlabeled inter-rank edges used to stretch to three
        // blank connector rows; a compact band is one │ row then the ▼ row
        let out = render("graph TD; A --> B --> C");
        let rows = gap_rows(&out, "A", "B");
        assert_eq!(
            rows.len(),
            2,
            "unlabeled band must be 2 rows (│ + ▼), got {rows:?}:\n{out}"
        );
        assert_eq!(rows[0].trim(), "│", "connector row: {out}");
        assert_eq!(rows[1].trim(), "▼", "arrow row: {out}");
    }

    #[test]
    fn labeled_edge_keeps_label_row() {
        let out = render("graph TD; A -->|yes| B --> C");
        let rows = gap_rows(&out, "A", "B");
        assert!(
            rows.len() > 2 && rows.iter().any(|r| r.contains("yes")),
            "labeled band must keep room for the label, got {rows:?}:\n{out}"
        );
        // The unlabeled band after B still compacts
        let rows = gap_rows(&out, "B", "C");
        assert_eq!(rows.len(), 2, "unlabeled band after labeled one:\n{out}");
    }

    #[test]
    fn loop_back_channel_survives_compact_bands() {
        // Feedback edge routes via the side channel; compact bands must not
        // break its re-entry arrowhead
        let out = render("graph TD; A --> B; B --> C; C --> A");
        assert!(out.contains('◄'), "loop-back re-entry arrowhead:\n{out}");
        let rows = gap_rows(&out, "A", "B");
        assert_eq!(rows.len(), 2, "compact band with loop-back present:\n{out}");
    }
}
