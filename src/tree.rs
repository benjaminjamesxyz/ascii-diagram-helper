use crate::color::Color;
use crate::schema::{TreeNodeSpec, TreeSpec};
use crate::stack::expand_tabs;
use crate::theme::{BoxStyle, Theme};

/// Paints `s` when `colored` and a color is set; otherwise returns it plain.
fn paint(colored: bool, color: Option<Color>, s: &str) -> String {
    match (colored, color) {
        (true, Some(c)) => c.paint(s),
        _ => s.to_string(),
    }
}

/// Style-specific tree connectors: `(tee, elbow, gutter)` — `├── ` for
/// non-last children, the elbow for the last child, and the vertical gutter
/// for ancestors with more siblings below. Every set draws only from its own
/// style's glyph family (no leakage): rounded/sharp share the thin tees and
/// horizontals their boxes already use, and differ in the elbow corner
/// (`╰` vs `└`); double and heavy use their full-weight families; ascii stays
/// pure 7-bit.
fn branch_glyphs(style: BoxStyle) -> (&'static str, &'static str, &'static str) {
    match style {
        BoxStyle::Rounded => ("├── ", "╰── ", "│   "),
        BoxStyle::Sharp => ("├── ", "└── ", "│   "),
        BoxStyle::Double => ("╠══ ", "╚══ ", "║   "),
        BoxStyle::Heavy => ("┣━━ ", "┗━━ ", "┃   "),
        BoxStyle::Ascii => ("|-- ", "\\-- ", "|   "),
    }
}

pub struct TreeRenderer<'a> {
    spec: &'a TreeSpec,
    theme: Theme,
}

impl<'a> TreeRenderer<'a> {
    #[must_use]
    pub fn new(spec: &'a TreeSpec, theme: Theme) -> Self {
        Self { spec, theme }
    }

    #[must_use]
    pub fn render(&self, colored: bool) -> String {
        let mut lines = Vec::new();

        // Tabs expand before rendering so terminals cannot reflow the line.
        let root_name = expand_tabs(&self.spec.root.name);
        let root_label = match &self.spec.root.annotation {
            Some(ann) => format!("{root_name} ({})", expand_tabs(ann)),
            None => root_name.into_owned(),
        };
        let root_line = paint(colored, self.spec.root.color, &root_label);
        lines.push(root_line);

        let num_children = self.spec.root.children.len();
        for (i, child) in self.spec.root.children.iter().enumerate() {
            let is_last = i + 1 == num_children;
            self.render_node(
                child,
                "",
                self.spec.root.color,
                is_last,
                colored,
                &mut lines,
            );
        }

        lines.join("\n")
    }

    fn render_node(
        &self,
        node: &TreeNodeSpec,
        prefix: &str,
        inherited: Option<Color>,
        is_last: bool,
        colored: bool,
        lines: &mut Vec<String>,
    ) {
        // A node's own color overrides the ancestor's for its subtree glyphs
        let effective = node.color.or(inherited);

        let (tee, elbow, gutter) = branch_glyphs(self.theme.box_style);
        let branch = if is_last { elbow } else { tee };

        let name = expand_tabs(&node.name);
        let label = match &node.annotation {
            Some(ann) => format!("{name} ({})", expand_tabs(ann)).into(),
            None => name,
        };

        let child_prefix = if is_last { "    " } else { gutter };
        let new_prefix = format!("{prefix}{}", paint(colored, effective, child_prefix));
        for (i, line) in label.split('\n').enumerate() {
            lines.push(if i == 0 {
                format!("{prefix}{}{line}", paint(colored, effective, branch))
            } else {
                format!("{new_prefix}{line}")
            });
        }

        let num_children = node.children.len();
        for (i, child) in node.children.iter().enumerate() {
            let child_is_last = i + 1 == num_children;
            self.render_node(child, &new_prefix, effective, child_is_last, colored, lines);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tree_rendering() {
        let spec = TreeSpec {
            style: BoxStyle::Rounded,
            root: TreeNodeSpec {
                name: "src/".to_string(),
                annotation: None,
                color: None,
                children: vec![
                    TreeNodeSpec {
                        name: "main.rs".to_string(),
                        annotation: Some("entry point".to_string()),
                        color: None,
                        children: vec![],
                    },
                    TreeNodeSpec {
                        name: "canvas.rs".to_string(),
                        annotation: None,
                        color: None,
                        children: vec![],
                    },
                ],
            },
        };

        let renderer = TreeRenderer::new(&spec, Theme::new(BoxStyle::Rounded));
        let out = renderer.render(false);
        assert!(out.contains("src/"));
        assert!(out.contains("├── main.rs (entry point)"));
        assert!(out.contains("╰── canvas.rs"));
        assert!(!out.contains('\u{1b}'), "plain tree has no ANSI");
    }

    #[test]
    fn test_tree_color_inheritance() {
        let spec = TreeSpec {
            style: BoxStyle::Sharp,
            root: TreeNodeSpec {
                name: "root".to_string(),
                annotation: None,
                color: None,
                children: vec![TreeNodeSpec {
                    name: "dir".to_string(),
                    annotation: None,
                    color: Some(Color::Red),
                    children: vec![TreeNodeSpec {
                        name: "leaf".to_string(),
                        annotation: None,
                        color: None,
                        children: vec![],
                    }],
                }],
            },
        };

        let renderer = TreeRenderer::new(&spec, Theme::new(BoxStyle::Sharp));
        let plain = renderer.render(false);
        let colored = renderer.render(true);
        assert_eq!(
            crate::color::Color::strip_ansi(&colored),
            plain,
            "color never shifts layout"
        );
        // leaf inherits red from "dir": its branch glyph is painted
        // (prefix and branch are separate paint runs — reset between them)
        assert!(
            colored.contains("\u{1b}[31m└── \u{1b}[39mleaf"),
            "inherited color: {colored:?}"
        );
    }

    #[test]
    fn test_tree_root_color_inheritance() {
        let spec = TreeSpec {
            style: BoxStyle::Rounded,
            root: TreeNodeSpec {
                name: "src/".to_string(),
                annotation: Some("project root".to_string()),
                color: Some(Color::Blue),
                children: vec![
                    TreeNodeSpec {
                        name: "main.rs".to_string(),
                        annotation: None,
                        color: None,
                        children: vec![],
                    },
                    TreeNodeSpec {
                        name: "lib.rs".to_string(),
                        annotation: None,
                        color: None,
                        children: vec![],
                    },
                ],
            },
        };

        let renderer = TreeRenderer::new(&spec, Theme::new(BoxStyle::Rounded));
        let colored = renderer.render(true);
        // Child branch glyph ├── inherits blue from root
        assert!(
            colored.contains("\u{1b}[34m├── \u{1b}[39mmain.rs"),
            "child branch painted blue: {colored:?}"
        );
    }

    #[test]
    fn test_tree_root_line_colored() {
        let spec = TreeSpec {
            style: BoxStyle::Rounded,
            root: TreeNodeSpec {
                name: "src/".to_string(),
                annotation: Some("project root".to_string()),
                color: Some(Color::Blue),
                children: vec![],
            },
        };

        let renderer = TreeRenderer::new(&spec, Theme::new(BoxStyle::Rounded));
        let colored = renderer.render(true);
        assert!(
            colored.contains("\u{1b}[34msrc/ (project root)\u{1b}[39m"),
            "root label receives ANSI color code: {colored:?}"
        );
    }

    #[test]
    fn test_tree_root_plain() {
        let spec = TreeSpec {
            style: BoxStyle::Rounded,
            root: TreeNodeSpec {
                name: "src/".to_string(),
                annotation: Some("project root".to_string()),
                color: Some(Color::Blue),
                children: vec![TreeNodeSpec {
                    name: "main.rs".to_string(),
                    annotation: None,
                    color: None,
                    children: vec![],
                }],
            },
        };

        let renderer = TreeRenderer::new(&spec, Theme::new(BoxStyle::Rounded));
        let plain = renderer.render(false);
        assert!(
            !plain.contains('\u{1b}'),
            "plain tree has no ANSI: {plain:?}"
        );
        assert!(plain.contains("src/ (project root)"));
        assert!(plain.contains("╰── main.rs"));
    }

    #[test]
    fn test_tree_styles_have_distinct_glyph_sets() {
        let spec_for = |style| TreeSpec {
            style,
            root: TreeNodeSpec {
                name: "root".to_string(),
                annotation: None,
                color: None,
                children: vec![
                    TreeNodeSpec {
                        name: "a".to_string(),
                        annotation: None,
                        color: None,
                        children: vec![TreeNodeSpec {
                            name: "a1".to_string(),
                            annotation: None,
                            color: None,
                            children: vec![],
                        }],
                    },
                    TreeNodeSpec {
                        name: "b".to_string(),
                        annotation: None,
                        color: None,
                        children: vec![],
                    },
                ],
            },
        };

        let renders: Vec<(BoxStyle, String)> = [
            BoxStyle::Rounded,
            BoxStyle::Sharp,
            BoxStyle::Double,
            BoxStyle::Heavy,
            BoxStyle::Ascii,
        ]
        .into_iter()
        .map(|s| {
            let r = TreeRenderer::new(&spec_for(s), Theme::new(s)).render(false);
            (s, r)
        })
        .collect();

        // All 5 styles render distinctly (TREE-02 / OUT-02).
        for (i, (_, a)) in renders.iter().enumerate() {
            for (_, b) in renders.iter().skip(i + 1) {
                assert_ne!(a, b, "styles {i} and beyond collide: {a:?} vs {b:?}");
            }
        }

        let of = |s: BoxStyle| -> &str {
            renders
                .iter()
                .find(|(st, _)| *st == s)
                .map(|(_, r)| r.as_str())
                .expect("style present")
        };

        let rounded = of(BoxStyle::Rounded);
        assert!(rounded.contains("├── "));
        assert!(rounded.contains("╰── "));
        assert!(rounded.contains("│   "));

        let sharp = of(BoxStyle::Sharp);
        assert!(sharp.contains("├── "));
        assert!(sharp.contains("└── "));
        assert!(sharp.contains("│   "));

        let double = of(BoxStyle::Double);
        assert!(double.contains("╠══ "));
        assert!(double.contains("╚══ "));
        assert!(double.contains("║   "));
        assert!(!double.contains('├') && !double.contains('└') && !double.contains("│   "));

        let heavy = of(BoxStyle::Heavy);
        assert!(heavy.contains("┣━━ "));
        assert!(heavy.contains("┗━━ "));
        assert!(heavy.contains("┃   "));
        assert!(!heavy.contains('├') && !heavy.contains('└') && !heavy.contains("│   "));

        let ascii = of(BoxStyle::Ascii);
        assert!(ascii.contains("|-- "));
        assert!(ascii.contains("\\-- "));
        assert!(ascii.contains("|   "));
        assert!(
            ascii.bytes().all(|b| b < 0x80),
            "ascii tree must be pure 7-bit: {ascii:?}"
        );
    }

    #[test]
    fn test_tree_tab_in_label_expands() {
        let spec = TreeSpec {
            style: BoxStyle::Rounded,
            root: TreeNodeSpec {
                name: "a\tb".to_string(),
                annotation: Some("x\ty".to_string()),
                color: None,
                children: vec![],
            },
        };
        let out = TreeRenderer::new(&spec, Theme::new(BoxStyle::Rounded)).render(false);
        assert!(!out.contains('\t'), "raw tab must not survive: {out:?}");
        assert!(
            out.contains("a   b (x   y)"),
            "4-col-stop expansion: {out:?}"
        );
    }

    #[test]
    fn test_multiline_child_names_and_annotations_keep_style_gutters() {
        for (style, tee, elbow, gutter) in [
            (BoxStyle::Rounded, "├── ", "╰── ", "│   "),
            (BoxStyle::Sharp, "├── ", "└── ", "│   "),
            (BoxStyle::Double, "╠══ ", "╚══ ", "║   "),
            (BoxStyle::Heavy, "┣━━ ", "┗━━ ", "┃   "),
            (BoxStyle::Ascii, "|-- ", "\\-- ", "|   "),
        ] {
            for (name, annotation, first, continuation) in [
                ("one\ntwo", None, "one", "two"),
                ("item", Some("a\nb"), "item (a", "b)"),
            ] {
                let spec = TreeSpec {
                    style,
                    root: TreeNodeSpec {
                        name: "root".to_string(),
                        annotation: None,
                        color: None,
                        children: vec![
                            TreeNodeSpec {
                                name: name.to_string(),
                                annotation: annotation.map(str::to_string),
                                color: None,
                                children: vec![],
                            },
                            TreeNodeSpec {
                                name: "last".to_string(),
                                annotation: None,
                                color: None,
                                children: vec![],
                            },
                        ],
                    },
                };
                assert_eq!(
                    TreeRenderer::new(&spec, Theme::new(style)).render(false),
                    format!("root\n{tee}{first}\n{gutter}{continuation}\n{elbow}last")
                );
            }
        }
    }

    #[test]
    fn test_multiline_nested_empty_segments_and_inherited_colors() {
        let spec = TreeSpec {
            style: BoxStyle::Rounded,
            root: TreeNodeSpec {
                name: "root".to_string(),
                annotation: None,
                color: Some(Color::Blue),
                children: vec![
                    TreeNodeSpec {
                        name: "dir\n".to_string(),
                        annotation: None,
                        color: None,
                        children: vec![TreeNodeSpec {
                            name: "\nleaf\n\n".to_string(),
                            annotation: None,
                            color: Some(Color::Red),
                            children: vec![],
                        }],
                    },
                    TreeNodeSpec {
                        name: "last\nend\n".to_string(),
                        annotation: None,
                        color: None,
                        children: vec![],
                    },
                ],
            },
        };
        let renderer = TreeRenderer::new(&spec, Theme::new(spec.style));
        let plain = renderer.render(false);
        assert_eq!(
            plain,
            "root\n├── dir\n│   \n│   ╰── \n│       leaf\n│       \n│       \n╰── last\n    end\n    "
        );
        let colored = renderer.render(true);
        assert_eq!(Color::strip_ansi(&colored), plain);
        assert!(colored.contains("\n\u{1b}[34m│   \u{1b}[39m\n"));
        assert!(colored.contains("\n\u{1b}[34m│   \u{1b}[39m\u{1b}[31m    \u{1b}[39mleaf\n"));
        assert!(colored.ends_with("\n\u{1b}[34m    \u{1b}[39m"));
    }

    #[test]
    fn test_multiline_root_and_child_tabs_preserve_physical_lines() {
        let spec = TreeSpec {
            style: BoxStyle::Rounded,
            root: TreeNodeSpec {
                name: "root\n".to_string(),
                annotation: None,
                color: None,
                children: vec![TreeNodeSpec {
                    name: "a\tb\n".to_string(),
                    annotation: Some("x\ty\n".to_string()),
                    color: None,
                    children: vec![],
                }],
            },
        };
        assert_eq!(
            TreeRenderer::new(&spec, Theme::new(spec.style)).render(false),
            "root\n\n╰── a   b\n     (x   y\n    )"
        );
    }
}
