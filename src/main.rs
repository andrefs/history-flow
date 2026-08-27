use clap::{Parser, Subcommand};

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

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Commands::Probe(args) => {
            println!("probe: not implemented yet");
            if let Some(t) = args.target {
                println!("  target: {}", t);
            }
            if let Some(c) = args.config {
                println!("  config: {}", c);
            }
            if args.json {
                println!("  --json");
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
