use clap::{Parser, Subcommand, ValueEnum};
use history_flow::config::{AttributionMode, Config, ImportMode, MatchMode, Source};

#[derive(Parser)]
#[command(
    name = "history-flow",
    version,
    about = "History Flow visualization for Wikipedia and Git"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Probe a source for revision count
    Probe(ProbeArgs),

    /// Produce only the Vega-Lite JSON spec
    Json(JsonArgs),

    /// Render the Vega-Lite chart (default HTML, or --format json|svg|png)
    Render(RenderArgs),

    /// Start the web server
    Serve(ServeArgs),
}

#[derive(Parser)]
struct ProbeArgs {
    #[command(flatten)]
    pipeline: PipelineFlags,

    /// Source URL, Wikipedia title, or GitHub file URL
    #[arg(value_name = "URL_OR_TITLE")]
    target: Option<String>,

    /// Config file path
    #[arg(long, value_name = "PATH")]
    config: Option<String>,

    /// Output as JSON
    #[arg(long)]
    json: bool,
}

#[derive(Parser)]
struct JsonArgs {
    #[command(flatten)]
    pipeline: PipelineFlags,

    #[arg(value_name = "URL_OR_TITLE")]
    target: Option<String>,

    #[arg(long, value_name = "PATH")]
    config: Option<String>,

    /// Output file
    #[arg(short, long, value_name = "FILE")]
    output: Option<String>,
}

#[derive(Clone, Copy, ValueEnum)]
enum RenderFormat {
    Json,
    Svg,
    Png,
}

#[derive(Parser)]
struct RenderArgs {
    #[command(flatten)]
    pipeline: PipelineFlags,

    #[arg(value_name = "URL_OR_TITLE")]
    target: Option<String>,

    #[arg(long, value_name = "PATH")]
    config: Option<String>,

    /// Output format: json (Vega-Lite spec), svg, png; default is HTML
    #[arg(long, value_enum)]
    format: Option<RenderFormat>,

    /// Output file
    #[arg(short, long, value_name = "FILE")]
    output: Option<String>,
}

#[derive(Parser)]
struct ServeArgs {
    /// Host:port to listen on
    #[arg(default_value = "127.0.0.1:8080", value_name = "HOST:PORT")]
    addr: String,
}

#[derive(clap::Args)]
struct PipelineFlags {
    /// Wikipedia or GitHub URL identifying the source.
    #[arg(long)]
    url: Option<String>,

    /// Source backend: wikipedia | git.
    #[arg(long)]
    source: Option<Source>,

    /// Wikipedia title or path to one tracked git file.
    #[arg(long)]
    page: Option<String>,

    /// Revision selection: all | last | nth.
    #[arg(long)]
    mode: Option<ImportMode>,

    /// N when --mode last.
    #[arg(long)]
    last: Option<usize>,

    /// N when --mode nth.
    #[arg(long)]
    nth: Option<usize>,

    /// Attribution mode: provenance | last_editor.
    #[arg(long)]
    attr_mode: Option<AttributionMode>,

    /// Text re-link matching: exact | fuzzy.
    #[arg(long)]
    match_mode: Option<MatchMode>,

    /// Similarity threshold when --match-mode fuzzy.
    #[arg(long)]
    fuzzy_thresh: Option<f64>,
}

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Commands::Probe(args) => {
            let cfg = config_from_flags(args.pipeline, args.target);
            match history_flow::import::probe(&cfg) {
                Ok(p) if args.json => println!(
                    "{}",
                    serde_json::to_string_pretty(&p).expect("probe must serialize")
                ),
                Ok(p) => {
                    println!("source:      {:?}", p.source);
                    println!("revisions:   {}", p.revision_count);
                    println!(
                        "newest:      {}",
                        p.newest_revision
                            .map(|d| d.to_rfc3339())
                            .unwrap_or_default()
                    );
                    println!(
                        "oldest:      {}",
                        p.oldest_revision
                            .map(|d| d.to_rfc3339())
                            .unwrap_or_default()
                    );
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    std::process::exit(1);
                }
            }
        }

        Commands::Json(args) => {
            let cfg = config_from_flags(args.pipeline, args.target);
            let grid = match run_pipeline(&cfg) {
                Ok(g) => g,
                Err(e) => {
                    eprintln!("error: {e}");
                    std::process::exit(1);
                }
            };
            match args.output {
                Some(path) => {
                    let json = serde_json::to_string_pretty(&grid).expect("grid must serialize");
                    std::fs::write(&path, json).expect("write output file");
                }
                None => println!(
                    "{}",
                    serde_json::to_string_pretty(&grid).expect("grid must serialize")
                ),
            }
        }

        Commands::Render(args) => {
            let cfg = config_from_flags(args.pipeline, args.target);
            let grid = match run_pipeline(&cfg) {
                Ok(g) => g,
                Err(e) => {
                    eprintln!("error: {e}");
                    std::process::exit(1);
                }
            };
            let spec = history_flow::visualize::build_spec(&grid);

            match args.format {
                None => {
                    let html = history_flow::visualize::html_page(&spec);
                    match args.output {
                        Some(path) => std::fs::write(&path, html).expect("write output file"),
                        None => println!("{html}"),
                    }
                }
                Some(RenderFormat::Json) => {
                    let out = serde_json::to_string_pretty(&spec).expect("spec must serialize");
                    match args.output {
                        Some(path) => std::fs::write(&path, out).expect("write output file"),
                        None => println!("{out}"),
                    }
                }
                Some(RenderFormat::Svg) => {
                    eprintln!("error: --format svg is not implemented yet");
                    std::process::exit(1);
                }
                Some(RenderFormat::Png) => {
                    eprintln!("error: --format png is not implemented yet");
                    std::process::exit(1);
                }
            }
        }

        Commands::Serve(args) => {
            println!("serve: not implemented yet");
            println!("  listening on {}", args.addr);
        }
    }
}

fn config_from_flags(f: PipelineFlags, target: Option<String>) -> Config {
    let mut c = Config::default();
    c.import.source = f.source.or(c.import.source);
    c.import.page = f.page.or(c.import.page);
    c.import.url = f.url.or(c.import.url);

    // Positional target acts as implicit --url if nothing else specified
    if c.import.url.is_none() && c.import.source.is_none() && c.import.page.is_none() {
        c.import.url = target;
    }

    c.import.mode = f.mode.unwrap_or(c.import.mode);
    c.import.last = f.last.unwrap_or(c.import.last);
    c.import.nth = f.nth.unwrap_or(c.import.nth);
    c.attribution.mode = f.attr_mode.unwrap_or(c.attribution.mode);
    c.attribution.match_mode = f.match_mode.unwrap_or(c.attribution.match_mode);
    c.attribution.fuzzy_thresh = f.fuzzy_thresh.unwrap_or(c.attribution.fuzzy_thresh);
    c
}

/// Run import -> select -> diff -> attribution, yielding an AuthorGrid.
fn run_pipeline(cfg: &Config) -> Result<history_flow::attribution::AuthorGrid, String> {
    let revisions = history_flow::import::import_revisions(cfg).map_err(|e| e.to_string())?;
    let revisions = history_flow::import::select_revisions(
        revisions,
        cfg.import.mode,
        cfg.import.last,
        cfg.import.nth,
    );
    let contents: Vec<Vec<String>> = revisions
        .iter()
        .map(|r| r.content.lines().map(|s| s.to_string()).collect())
        .collect();
    let diffs: Vec<Vec<history_flow::attribution::DiffOp>> = contents
        .windows(2)
        .map(|w| history_flow::attribution::diff::diff_lines(&w[0], &w[1]))
        .collect();
    history_flow::attribution::run_attribution(&revisions, &diffs).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_override_defaults() {
        let cli = Cli::try_parse_from([
            "history-flow",
            "probe",
            "--mode",
            "nth",
            "--last",
            "42",
            "--attr-mode",
            "last_editor",
        ])
        .unwrap();
        let Commands::Probe(args) = cli.command else {
            panic!("expected probe command");
        };
        let cfg = config_from_flags(args.pipeline, args.target);
        assert_eq!(cfg.import.mode, ImportMode::Nth);
        assert_eq!(cfg.import.last, 42);
        assert_eq!(cfg.import.nth, 5);
        assert_eq!(cfg.attribution.mode, AttributionMode::LastEditor);
        assert_eq!(cfg.attribution.match_mode, MatchMode::Exact);
    }
}
