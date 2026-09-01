/// Demonstrates the History Flow library API end-to-end.
///
/// Usage:
///   cargo run --example lib_usage                           # Wikipedia "Evolution"
///   cargo run --example lib_usage -- "Climate change"       # custom title
///   cargo run --example lib_usage -- --json-only            # Vega-Lite spec only, no HTML wrapper
///   cargo run --example lib_usage -- --source git \
///       --repo /path/to/repo --page notes.txt              # local git file
use std::env;
use std::process;

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let json_only = args.iter().any(|a| a == "--json-only");
    let positional: Option<String> = args.into_iter().filter(|a| !a.starts_with('-')).next();

    let mut config = history_flow::config::Config::default();
    if let Some(target) = positional {
        config.import.url = Some(target);
    }

    // 1. Probe
    eprintln!("probing...");
    let probe = history_flow::import::probe(&config).unwrap_or_else(|e| {
        eprintln!("error: {e}");
        process::exit(1);
    });
    eprintln!("  {} revisions", probe.revision_count);

    // 2. Import
    eprintln!("importing...");
    let revisions = history_flow::import::import_revisions(&config).unwrap_or_else(|e| {
        eprintln!("error: {e}");
        process::exit(1);
    });
    eprintln!("  {} fetched", revisions.len());

    // 3. Select (default: all, capped at 200)
    let revisions = history_flow::import::select_revisions(
        revisions,
        config.import.mode,
        config.import.last,
        config.import.nth,
    );
    eprintln!("  {} after selection", revisions.len());

    // 4. Diff chain
    let contents: Vec<Vec<String>> = revisions
        .iter()
        .map(|r| r.content.lines().map(String::from).collect())
        .collect();
    let diffs: Vec<Vec<history_flow::attribution::DiffOp>> = contents
        .windows(2)
        .map(|w| history_flow::attribution::diff::diff_lines(&w[0], &w[1]))
        .collect();

    // 5. Attribution
    eprintln!("attributing...");
    let grid = history_flow::attribution::run_attribution(&revisions, &diffs).unwrap_or_else(|e| {
        eprintln!("error: {e}");
        process::exit(1);
    });

    // 6. Visualize
    let spec = history_flow::visualize::build_spec(&grid);
    let html = history_flow::visualize::html_page(&spec);
    if json_only {
        println!("{}", spec);
    } else {
        println!("{html}");
    }
    eprintln!("done");
}
