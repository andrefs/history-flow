# History Flow

A from-scratch, opinionated Rust re-implementation of IBM's *History Flow*: a
visualization of how a document evolved across revisions — a Wikipedia page or a
single Git-tracked file — drawn as an author-colored stacked-bar chart.

Each column is one revision; column height is proportional to content size;
color is the contributing author. Attribution uses a **provenance graph**, so
deleted-then-restored (reverted) text re-links to its **original author** — the
piece `git blame` cannot do.

## Screenshots

(to be added)

## Delivery forms

- **Library** — the pipeline stages (`import` → `attribution` → `visualize`) are
  public functions in the `history_flow` crate, usable from the CLI, a web
  server, or your own code.
- **CLI** — subcommands: `probe`, `json`, `render`, `serve`.
- **Web** — `serve` starts an HTTP server rendering the configured target's
  chart (the interactive single-input page is planned, not yet built).

## Quick start

```
cargo install --path .

# Size up a source first (see below) — then render it.
history-flow render "Evolution"                        # Wikipedia title
history-flow render https://en.wikipedia.org/wiki/Evolution
history-flow render --source git --repo /path/to/repo --page notes.txt
history-flow render --source git --repo owner/repo --page README.md --mode last --last 50
history-flow serve --source git --repo /path/to/repo --page notes.txt   # → http://127.0.0.1:8080
```

`render` emits a self-contained HTML page (Vega-Lite spec + bundled JS inlined,
no CDN) on stdout, or use `--format json` for the raw spec / `-o file` to write
it to disk.

## Size up a source first

`probe` reports the revision count and time range *without* downloading full
content — use it to decide `--mode all` vs `last=N` / `nth=N` before a heavy
render:

```
history-flow probe "Evolution"
history-flow probe --source git --repo /path/to/repo --page notes.txt
history-flow probe --json https://github.com/owner/repo/blob/main/README.md
```

`--mode all` renders every revision (`last=200` / `nth=5` defaults bound the
cost). Large or long-lived sources benefit from `--mode last --last 100`.

## CLI reference

```
history-flow <probe|json|render|serve> [flags] [<url-or-title>]
```

Global source flags (shared by all pipeline subcommands):

| flag | meaning |
| --- | --- |
| `--url <URL>` | Wikipedia or GitHub URL |
| `--source wikipedia\|git` | source backend |
| `--page <STRING>` | Wikipedia title, or one tracked git file |
| `--repo <REPO>` | git: local path or owner/repo |
| `--mode all\|last\|nth` | revision selection |
| `--last <N>` | N when `--mode last` (default 200) |
| `--nth <N>` | N when `--mode nth` (default 5) |
| `--attr-mode provenance\|last_editor` | attribution mode |
| `--match-mode exact\|fuzzy` | text re-link matching |
| `--fuzzy-thresh <FLOAT>` | similarity for fuzzy re-link (default 0.95) |

If no flag names a source, the positional argument is used: a GitHub
`blob/<rev>/<path>` URL → git; a Wikipedia URL or anything else → Wikipedia.
Both `--url` and `--source`/`--page` together is an error.

`json` prints the per-revision author grid (revision × line → `{author, size}`)
as JSON. `render` renders that grid as the chart; `serve` does the same over
HTTP. Structured output goes to stdout; progress and timings go to stderr.

## Defaults

| setting | default |
| --- | --- |
| `source` / `page` | unset |
| `mode` | `all` |
| `last` | 200 |
| `nth` | 5 |
| `attr-mode` | `provenance` |
| `match-mode` | `exact` |
| `fuzzy-thresh` | 0.95 |

A `config.toml` file-loader (discovery of `--config`, `./config.toml`,
`~/.config/history-flow/config.toml`) is planned; flags currently override these
built-in defaults.

## License

Copyright © 2026 André Santos

MIT — see `LICENSE`.

## Acknowledgements / Related work

- **IBM's History Flow** — the original visualization this project re-implements.
- The research paper: Fernanda B. Viégas & Martin Wattenberg, *History Flow:
  Results on the Analysis and Manipulation of Wiki Histories*, CHI 2004.
- https://github.com/rdmpage/wikihistoryflow — a related implementation.
- Jeff Atwood, *Mixing Oil and Water — Authorship in a Wiki World*,
  https://blog.codinghorror.com/mixing-oil-and-water-authorship-in-a-wiki-world/

See `.opencode/PLAN.md` for the architecture and roadmap.