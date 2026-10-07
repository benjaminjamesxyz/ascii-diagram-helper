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
                    color: None,
                },
                NodeSpec {
                    id: "B".to_string(),
                    label: "Server".to_string(),
                    shape: NodeShape::Box,
                    dashed_border: false,
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
                    color: None,
                },
                NodeSpec {
                    id: "B".to_string(),
                    label: "End".to_string(),
                    shape: NodeShape::Box,
                    dashed_border: false,
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
                    color: None,
                },
                NodeSpec {
                    id: "B".to_string(),
                    label: "Is Valid?".to_string(),
                    shape: NodeShape::Diamond,
                    dashed_border: false,
                    color: None,
                },
                NodeSpec {
                    id: "C".to_string(),
                    label: "Proceed".to_string(),
                    shape: NodeShape::Box,
                    dashed_border: false,
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
                    color: None,
                },
                NodeSpec {
                    id: "Sum".to_string(),
                    label: "Error".to_string(),
                    shape: NodeShape::Circle,
                    dashed_border: false,
                    color: None,
                },
                NodeSpec {
                    id: "PID".to_string(),
                    label: "Controller".to_string(),
                    shape: NodeShape::Box,
                    dashed_border: false,
                    color: None,
                },
                NodeSpec {
                    id: "Plant".to_string(),
                    label: "Motor".to_string(),
                    shape: NodeShape::Box,
                    dashed_border: false,
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
                    color: None,
                },
                NodeSpec {
                    id: "A".into(),
                    label: "Task A".into(),
                    shape: NodeShape::Box,
                    dashed_border: false,
                    color: None,
                },
                NodeSpec {
                    id: "B".into(),
                    label: "Task B".into(),
                    shape: NodeShape::Box,
                    dashed_border: false,
                    color: None,
                },
                NodeSpec {
                    id: "C".into(),
                    label: "Task C".into(),
                    shape: NodeShape::Box,
                    dashed_border: false,
                    color: None,
                },
                NodeSpec {
                    id: "Z".into(),
                    label: "Done".into(),
                    shape: NodeShape::Box,
                    dashed_border: false,
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
                    color: None,
                },
                NodeSpec {
                    id: "B".into(),
                    label: "End".into(),
                    shape: NodeShape::Box,
                    dashed_border: false,
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
                    color: None,
                },
                NodeSpec {
                    id: "B".into(),
                    label: "End".into(),
                    shape: NodeShape::Box,
                    dashed_border: false,
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
