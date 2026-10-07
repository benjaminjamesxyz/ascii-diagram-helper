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
                    thick_border: false,
                    color: None,
                },
                NodeSpec {
                    id: "B".to_string(),
                    label: "Server".to_string(),
                    shape: NodeShape::Box,
                    dashed_border: false,
                    fill_color: None,
                    thick_border: false,
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
                    thick_border: false,
                    color: None,
                },
                NodeSpec {
                    id: "B".to_string(),
                    label: "End".to_string(),
                    shape: NodeShape::Box,
                    dashed_border: false,
                    fill_color: None,
                    thick_border: false,
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
                    thick_border: false,
                    color: None,
                },
                NodeSpec {
                    id: "B".to_string(),
                    label: "Is Valid?".to_string(),
                    shape: NodeShape::Diamond,
                    dashed_border: false,
                    fill_color: None,
                    thick_border: false,
                    color: None,
                },
                NodeSpec {
                    id: "C".to_string(),
                    label: "Proceed".to_string(),
                    shape: NodeShape::Box,
                    dashed_border: false,
                    fill_color: None,
                    thick_border: false,
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
        assert!(result.contains("╔"));
        assert!(result.contains("║"));
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
                    thick_border: false,
                    color: None,
                },
                NodeSpec {
                    id: "Sum".to_string(),
                    label: "Error".to_string(),
                    shape: NodeShape::Circle,
                    dashed_border: false,
                    fill_color: None,
                    thick_border: false,
                    color: None,
                },
                NodeSpec {
                    id: "PID".to_string(),
                    label: "Controller".to_string(),
                    shape: NodeShape::Box,
                    dashed_border: false,
                    fill_color: None,
                    thick_border: false,
                    color: None,
                },
                NodeSpec {
                    id: "Plant".to_string(),
                    label: "Motor".to_string(),
                    shape: NodeShape::Box,
                    dashed_border: false,
                    fill_color: None,
                    thick_border: false,
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
        let mut nodes = renderer.prepare_nodes();
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
                thick_border: false,
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
                    thick_border: false,
                    color: None,
                },
                NodeSpec {
                    id: "A".into(),
                    label: "Task A".into(),
                    shape: NodeShape::Box,
                    dashed_border: false,
                    fill_color: None,
                    thick_border: false,
                    color: None,
                },
                NodeSpec {
                    id: "B".into(),
                    label: "Task B".into(),
                    shape: NodeShape::Box,
                    dashed_border: false,
                    fill_color: None,
                    thick_border: false,
                    color: None,
                },
                NodeSpec {
                    id: "C".into(),
                    label: "Task C".into(),
                    shape: NodeShape::Box,
                    dashed_border: false,
                    fill_color: None,
                    thick_border: false,
                    color: None,
                },
                NodeSpec {
                    id: "Z".into(),
                    label: "Done".into(),
                    shape: NodeShape::Box,
                    dashed_border: false,
                    fill_color: None,
                    thick_border: false,
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
                    thick_border: false,
                    color: None,
                },
                NodeSpec {
                    id: "B".into(),
                    label: "End".into(),
                    shape: NodeShape::Box,
                    dashed_border: false,
                    fill_color: None,
                    thick_border: false,
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
                    thick_border: false,
                    color: None,
                },
                NodeSpec {
                    id: "B".into(),
                    label: "End".into(),
                    shape: NodeShape::Box,
                    dashed_border: false,
                    fill_color: None,
                    thick_border: false,
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
        assert_eq!(
            x_col, y_col,
            "X and Y on the same row (LR inside subgraph)"
        );
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
                thick_border: true,
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
                thick_border: false,
            }],
            edges: Vec::new(),
        };
        let theme = crate::theme::Theme::new(BoxStyle::Sharp);
        let out = FlowchartRenderer::new(&spec, theme.clone()).render(true);
        assert!(out.contains("\u{1b}[31mHot"), "red SGR around label in {out:?}");
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
