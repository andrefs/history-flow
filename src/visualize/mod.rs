//! Visualize: AuthorGrid -> Vega-Lite spec.

use crate::attribution::AuthorGrid;
use serde_json::json;

const VEGA: &str = include_str!("../../assets/js/vega.min.js");
const VEGA_LITE: &str = include_str!("../../assets/js/vega-lite.min.js");
const VEGA_EMBED: &str = include_str!("../../assets/js/vega-embed.min.js");

/// One tidy data row for the chart.
#[derive(Debug, Clone)]
pub struct Datum {
    /// Revision index (column).
    pub revision: usize,

    /// Line index within the revision (stack order).
    pub line: usize,

    /// Author of this line in this revision.
    pub author: String,

    /// Line length in characters (segment height).
    pub size: usize,
}

/// Build the Vega-Lite stacked-bar spec from an AuthorGrid.
pub fn build_spec(grid: &AuthorGrid) -> serde_json::Value {
    let mut values = Vec::new();
    for (rev, row) in grid.grid.iter().enumerate() {
        for (line_idx, cell) in row.iter().enumerate() {
            values.push(json!({
                "revision": rev,
                "line": line_idx,
                "author": cell.author,
                "size": cell.size,
            }));
        }
    }
    // Width: sub-linear rule so per-column width narrows as revisions grow.
    let cols = grid.revisions;
    let width = (120.0_f64).max((cols as f64) * (cols as f64).sqrt() + (cols as f64) * 0.4);

    json!({
        "$schema": "https://vega.github.io/schema/vega-lite/v5.json",
        "width": width,
        "data": { "values": values },
        "mark": "bar",
        "encoding": {
            "x": { "field": "revision", "type": "ordinal", "scale": { "padding": 0 }, "axis": null },
            "y": { "field": "size", "type": "quantitative", "stack": true, "scale": { "reverse": true }, "axis": null },
            "color": { "field": "author", "type": "nominal" },
            "order": { "field": "line", "type": "ordinal" },
            "tooltip": [
                { "field": "author", "type": "nominal" },
                { "field": "size", "type": "quantitative" }
            ]
        }
    })
}

/// Wrap a Vega-Lite spec in a self-contained HTML page.
pub fn html_page(spec: &serde_json::Value) -> String {
    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>history-flow</title>
<style>body{{margin:0}}#view{{width:100%}}</style>
</head>
<body>
<div id="view"></div>
<script>{VEGA}</script>
<script>{VEGA_LITE}</script>
<script>{VEGA_EMBED}</script>
<script>
const spec = {spec};
vegaEmbed('#view', spec, {{renderer: 'svg'}}).catch(console.error);
</script>
</body>
</html>"#,
        spec = serde_json::to_string_pretty(spec).expect("spec must serialize"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attribution::{AuthorGrid, GridCell};

    fn sample_grid() -> AuthorGrid {
        AuthorGrid {
            revisions: 2,
            grid: vec![
                vec![
                    GridCell {
                        author: "Alice".into(),
                        size: 3,
                    },
                    GridCell {
                        author: "Alice".into(),
                        size: 5,
                    },
                ],
                vec![
                    GridCell {
                        author: "Bob".into(),
                        size: 3,
                    },
                    GridCell {
                        author: "Alice".into(),
                        size: 7,
                    },
                ],
            ],
        }
    }

    #[test]
    fn spec_has_one_row_per_cell() {
        let spec = build_spec(&sample_grid());
        let rows = spec["data"]["values"].as_array().unwrap();
        assert_eq!(rows.len(), 4);
    }

    #[test]
    fn spec_encoding_is_stacked_bar() {
        let spec = build_spec(&sample_grid());
        assert_eq!(spec["encoding"]["x"]["field"], "revision");
        assert_eq!(spec["encoding"]["x"]["type"], "ordinal");
        assert_eq!(spec["encoding"]["x"]["scale"]["padding"], 0);
        assert_eq!(spec["encoding"]["y"]["field"], "size");
        assert_eq!(spec["encoding"]["y"]["stack"], true);
        assert_eq!(spec["encoding"]["y"]["scale"]["reverse"], true);
        assert_eq!(spec["encoding"]["color"]["field"], "author");
        assert_eq!(spec["encoding"]["order"]["field"], "line");
    }

    #[test]
    fn width_grows_with_revisions() {
        let small = build_spec(&sample_grid());
        let small_w = small["width"].as_f64().unwrap();
        let big_grid = AuthorGrid {
            revisions: 100,
            grid: vec![vec![]; 100],
        };
        let big_w = build_spec(&big_grid)["width"].as_f64().unwrap();
        assert!(big_w > small_w);
    }

    #[test]
    fn html_page_embeds_spec_and_bundles() {
        let html = html_page(&build_spec(&sample_grid()));
        assert!(html.contains("vegaEmbed"));
        assert!(html.contains("\"revision\""));
    }
}
