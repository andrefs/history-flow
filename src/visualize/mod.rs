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

    /// Date of this revision (for tooltip).
    pub date: String,

    /// Line index within the revision (stack order).
    pub line: usize,

    /// Author of this line in this revision.
    pub author: String,

    /// Line length in characters (segment height).
    pub size: usize,
}

/// Build the Vega-Lite stacked-bar spec from an AuthorGrid.
pub fn build_spec(grid: &AuthorGrid) -> serde_json::Value {
    build_spec_with_title(grid, None)
}

/// Build the spec, optionally labeling the chart with `title` (shown in the
/// chart and therefore in SVG/PNG exports).
pub fn build_spec_with_title(grid: &AuthorGrid, title: Option<&str>) -> serde_json::Value {
    let mut values = Vec::new();
    let mut author_total: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    for row in grid.grid.iter() {
        for cell in row.iter() {
            *author_total.entry(cell.author.as_str()).or_insert(0) += cell.size;
        }
    }
    // Each revision becomes a horizontal band [x, x2). Within a band, lines are
    // stacked bottom-up (offset y..y2). Using a quantitative x band (instead of
    // an ordinal scale) is what makes wheel-zoom + pan via `bind: "scales"`
    // work reliably.
    for (rev, row) in grid.grid.iter().enumerate() {
        let x: f64 = rev as f64;
        let x2: f64 = x + 0.9;
        let mut offset: f64 = 0.0;
        for (line_idx, cell) in row.iter().enumerate() {
            let size = cell.size as f64;
            let y = offset;
            let y2 = offset + size;
            offset = y2;
            values.push(json!({
                "revision": rev,
                "date": grid.dates[rev].clone(),
                "line": line_idx,
                "author": cell.author,
                "size": cell.size,
                "author_total": author_total[cell.author.as_str()],
                "x": x,
                "x2": x2,
                "y": y,
                "y2": y2,
            }));
        }
    }
    // Width: one pixel per revision column (each bar is exactly 1px wide).
    let cols = grid.revisions;
    let width = (cols as f64).max(1.0);

    let mut spec = json!({
        "$schema": "https://vega.github.io/schema/vega-lite/v5.json",
        "width": width,
        "height": 600,
        "params": [
            {
                "name": "grid",
                "select": { "type": "interval", "encodings": ["x"], "zoom": "wheel" },
                "bind": "scales"
            }
        ],
        "data": { "values": values },
        "mark": "rect",
        "encoding": {
            "x": { "field": "x", "type": "quantitative", "axis": null },
            "x2": { "field": "x2" },
            "y": { "field": "y", "type": "quantitative", "axis": null, "scale": { "reverse": true } },
            "y2": { "field": "y2" },
            "color": { "field": "author", "type": "nominal", "sort": { "field": "author_total", "order": "descending" } },
            "order": { "field": "line", "type": "ordinal" },
            "tooltip": [
                { "field": "author", "type": "nominal" },
                { "field": "date", "type": "temporal" , "format": "%Y-%m-%d %H:%M:%S"},
                { "field": "size", "type": "quantitative" }
            ]
        }
    });
    if let Some(t) = title {
        spec["title"] = serde_json::json!({ "text": t, "anchor": "start" });
    }
    spec
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
            dates: vec![
                "2024-01-01T00:00:00+00:00".to_string(),
                "2024-01-02T00:00:00+00:00".to_string(),
            ],
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
    fn spec_encoding_is_stacked_bands() {
        let spec = build_spec(&sample_grid());
        assert_eq!(spec["encoding"]["x"]["field"], "x");
        assert_eq!(spec["encoding"]["x"]["type"], "quantitative");
        assert_eq!(spec["encoding"]["x2"]["field"], "x2");
        assert_eq!(spec["encoding"]["y"]["field"], "y");
        assert_eq!(spec["encoding"]["y2"]["field"], "y2");
        assert_eq!(spec["encoding"]["color"]["field"], "author");
        assert_eq!(spec["encoding"]["order"]["field"], "line");
        assert_eq!(spec["mark"], "rect");
    }

    #[test]
    fn build_spec_with_title_sets_title_field() {
        let spec = build_spec_with_title(&sample_grid(), Some("History of the potato"));
        assert_eq!(spec["title"]["text"], "History of the potato");
        assert_eq!(spec["title"]["anchor"], "start");

        let no_title = build_spec(&sample_grid());
        assert!(no_title["title"].is_null());
    }

    #[test]
    fn width_grows_with_revisions() {
        let small = build_spec(&sample_grid());
        let small_w = small["width"].as_f64().unwrap();
        let big_grid = AuthorGrid {
            revisions: 100,
            dates: (0..100)
                .map(|i| format!("2024-01-{:02}T00:00:00+00:00", i + 1))
                .collect(),
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

    #[test]
    fn spec_rows_include_revision_date() {
        let spec = build_spec(&sample_grid());
        let rows = spec["data"]["values"].as_array().unwrap();
        assert!(rows[0]["date"].as_str().unwrap().contains("2024-01-01"));
        let tooltip = spec["encoding"]["tooltip"].as_array().unwrap();
        assert!(tooltip.iter().any(|t| t["field"] == "date"));
    }
}
