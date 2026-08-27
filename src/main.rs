use clap::{Parser, Subcommand};
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
    /// Render the full pipeline (spec or HTML)
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

#[derive(Parser)]
struct RenderArgs {
    #[command(flatten)]
    pipeline: PipelineFlags,

    #[arg(value_name = "URL_OR_TITLE")]
    target: Option<String>,

    #[arg(long, value_name = "PATH")]
    config: Option<String>,

    /// Output as self-contained HTML
    #[arg(long)]
    html: bool,

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
            println!("json: not implemented yet");
            if let Some(t) = args.target {
                println!("  target: {}", t);
            }
            if let Some(c) = args.config {
                println!("  config: {}", c);
            }
            if let Some(o) = args.output {
                println!("  -o {}", o);
            }
        }

        Commands::Render(args) => {
            println!("render: not implemented yet");
            if let Some(t) = args.target {
                println!("  target: {}", t);
            }
            if let Some(c) = args.config {
                println!("  config: {}", c);
            }
            if args.html {
                println!("  --html");
            }
            if let Some(o) = args.output {
                println!("  -o {}", o);
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
