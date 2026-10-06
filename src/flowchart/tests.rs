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
            nodes: vec![
                NodeSpec {
                    id: "A".to_string(),
                    label: "Client".to_string(),
                    shape: NodeShape::Box,
                },
                NodeSpec {
                    id: "B".to_string(),
                    label: "Server".to_string(),
                    shape: NodeShape::Box,
                },
            ],
            edges: vec![EdgeSpec {
                from: "A".to_string(),
                to: "B".to_string(),
                label: Some("HTTP".to_string()),
                arrow: ArrowDirection::Forward,
                dashed: false,
            }],
        };

        let renderer = FlowchartRenderer::new(&spec, Theme::new(BoxStyle::Rounded));
        let result = renderer.render();
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
            nodes: vec![
                NodeSpec {
                    id: "A".to_string(),
                    label: "Start".to_string(),
                    shape: NodeShape::Box,
                },
                NodeSpec {
                    id: "B".to_string(),
                    label: "End".to_string(),
                    shape: NodeShape::Box,
                },
            ],
            edges: vec![EdgeSpec {
                from: "A".to_string(),
                to: "B".to_string(),
                label: None,
                arrow: ArrowDirection::Forward,
                dashed: false,
            }],
        };

        let renderer = FlowchartRenderer::new(&spec, Theme::ascii());
        let result = renderer.render();
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
            nodes: vec![
                NodeSpec {
                    id: "A".to_string(),
                    label: "Start".to_string(),
                    shape: NodeShape::Box,
                },
                NodeSpec {
                    id: "B".to_string(),
                    label: "Is Valid?".to_string(),
                    shape: NodeShape::Diamond,
                },
                NodeSpec {
                    id: "C".to_string(),
                    label: "Proceed".to_string(),
                    shape: NodeShape::Box,
                },
            ],
            edges: vec![
                EdgeSpec {
                    from: "A".to_string(),
                    to: "B".to_string(),
                    label: None,
                    arrow: ArrowDirection::Forward,
                    dashed: false,
                },
                EdgeSpec {
                    from: "B".to_string(),
                    to: "C".to_string(),
                    label: Some("Yes".to_string()),
                    arrow: ArrowDirection::Forward,
                    dashed: false,
                },
            ],
        };

        let renderer = FlowchartRenderer::new(&spec, Theme::new(BoxStyle::Rounded));
        let result = renderer.render();
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
            nodes: vec![
                NodeSpec {
                    id: "Ref".to_string(),
                    label: "Target".to_string(),
                    shape: NodeShape::Box,
                },
                NodeSpec {
                    id: "Sum".to_string(),
                    label: "Error".to_string(),
                    shape: NodeShape::Circle,
                },
                NodeSpec {
                    id: "PID".to_string(),
                    label: "Controller".to_string(),
                    shape: NodeShape::Box,
                },
                NodeSpec {
                    id: "Plant".to_string(),
                    label: "Motor".to_string(),
                    shape: NodeShape::Box,
                },
            ],
            edges: vec![
                EdgeSpec {
                    from: "Ref".to_string(),
                    to: "Sum".to_string(),
                    label: None,
                    arrow: ArrowDirection::Forward,
                    dashed: false,
                },
                EdgeSpec {
                    from: "Sum".to_string(),
                    to: "PID".to_string(),
                    label: None,
                    arrow: ArrowDirection::Forward,
                    dashed: false,
                },
                EdgeSpec {
                    from: "PID".to_string(),
                    to: "Plant".to_string(),
                    label: None,
                    arrow: ArrowDirection::Forward,
                    dashed: false,
                },
                EdgeSpec {
                    from: "Plant".to_string(),
                    to: "Sum".to_string(),
                    label: Some("Feedback".to_string()),
                    arrow: ArrowDirection::Forward,
                    dashed: false,
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

        let result = renderer.render();
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
            nodes: vec![NodeSpec {
                id: "A".to_string(),
                label: "Box".to_string(),
                shape: NodeShape::Box,
            }],
            edges: vec![EdgeSpec {
                from: "A".to_string(),
                to: "A".to_string(),
                label: Some("retry".to_string()),
                arrow: ArrowDirection::Forward,
                dashed: false,
            }],
        }
    }

    #[test]
    fn test_self_loop_tb() {
        let spec = spec(LayoutDirection::TB);
        let out = FlowchartRenderer::new(&spec, Theme::new(BoxStyle::Rounded)).render();
        assert!(out.contains("Box"));
        assert!(out.contains("retry"), "self-loop label must render");
        assert!(out.contains('◄'), "re-entry arrowhead must render");
    }

    #[test]
    fn test_self_loop_lr() {
        let spec = spec(LayoutDirection::LR);
        let out = FlowchartRenderer::new(&spec, Theme::new(BoxStyle::Rounded)).render();
        assert!(out.contains("Box"));
        assert!(out.contains("retry"));
        assert!(out.contains('◄'));
    }

    /// Regression: `A --> A` at x=0 used to panic with `attempt to subtract
    /// with overflow` in the same-rank branch.
    #[test]
    fn test_self_loop_at_origin_no_panic() {
        let spec = spec(LayoutDirection::TB);
        let out = FlowchartRenderer::new(&spec, Theme::new(BoxStyle::Rounded)).render();
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
            direction: LayoutDirection::TB,
            nodes: vec![
                NodeSpec {
                    id: "W".into(),
                    label: "Watchdog".into(),
                    shape: NodeShape::Box,
                },
                NodeSpec {
                    id: "A".into(),
                    label: "Task A".into(),
                    shape: NodeShape::Box,
                },
                NodeSpec {
                    id: "B".into(),
                    label: "Task B".into(),
                    shape: NodeShape::Box,
                },
                NodeSpec {
                    id: "C".into(),
                    label: "Task C".into(),
                    shape: NodeShape::Box,
                },
                NodeSpec {
                    id: "Z".into(),
                    label: "Done".into(),
                    shape: NodeShape::Box,
                },
            ],
            edges: vec![
                EdgeSpec {
                    from: "W".into(),
                    to: "A".into(),
                    label: None,
                    arrow: ArrowDirection::Forward,
                    dashed: true,
                },
                EdgeSpec {
                    from: "W".into(),
                    to: "B".into(),
                    label: None,
                    arrow: ArrowDirection::Forward,
                    dashed: true,
                },
                EdgeSpec {
                    from: "W".into(),
                    to: "C".into(),
                    label: None,
                    arrow: ArrowDirection::Forward,
                    dashed: true,
                },
                EdgeSpec {
                    from: "A".into(),
                    to: "Z".into(),
                    label: None,
                    arrow: ArrowDirection::Forward,
                    dashed: false,
                },
                EdgeSpec {
                    from: "B".into(),
                    to: "Z".into(),
                    label: None,
                    arrow: ArrowDirection::Forward,
                    dashed: false,
                },
                EdgeSpec {
                    from: "C".into(),
                    to: "Z".into(),
                    label: None,
                    arrow: ArrowDirection::Forward,
                    dashed: false,
                },
            ],
        };
        let r = FlowchartRenderer::new(&spec, crate::theme::Theme::new(BoxStyle::Rounded));
        let out = r.render();
        // Every target must have an arrowhead directly above its box
        let lines: Vec<&str> = out.lines().collect();
        let arrow_row = lines.iter().find(|l| l.contains('▼')).expect("arrow row");
        let count = arrow_row.matches('▼').count();
        assert_eq!(count, 3, "expected 3 drops, row: {arrow_row}");
    }
}
