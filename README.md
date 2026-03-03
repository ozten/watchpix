# watchpix

**A live-reloading image gallery server for remote and headless machines.**

`watchpix` watches a directory tree for image files, serves a web-based gallery sorted by recency, and live-updates the browser via WebSocket whenever images are created, modified, or deleted. Designed for developers and AI/ML practitioners who work on headless VMs over SSH.

![watchpix gallery screenshot](docs/screenshot.png)

## Install

```bash
cargo install --path .
```

## Usage

```bash
# Watch current directory, serve on port 8080
watchpix

# Watch a specific directory on a custom port
watchpix ./output --port 3000

# Add extra directories to the deny list
watchpix ./workspace --deny vendor,assets/raw

# Bind to all interfaces (for LAN access)
watchpix --bind 0.0.0.0

# Enable verbose logging
watchpix -v
```

Then on your local machine, tunnel in:

```bash
ssh -L 8080:localhost:8080 user@remote-host
# Open http://localhost:8080 in your browser
```

## Typical workflow

1. SSH into a remote VM.
2. Agents, scripts, or training jobs are generating images on that machine.
3. Run `watchpix` in the project root (or any parent directory).
4. SSH-tunnel the port to your local machine.
5. Open `localhost:8080` in a browser tab and leave it open.
6. Images appear in the gallery in real time as they are created or modified, newest first.
7. Click any image to view it full-resolution.

No `scp`. No X11 forwarding. No context switching.

## CLI options

```
watchpix [OPTIONS] [ROOT]

Arguments:
  [ROOT]    Directory to watch [default: .]

Options:
  -p, --port <PORT>     Port to listen on [default: 8080]
  -b, --bind <ADDR>     Address to bind to [default: 127.0.0.1]
  -d, --deny <DIRS>     Additional directories to deny, comma-separated
  -v, --verbose         Enable verbose logging
  -h, --help            Print help
  -V, --version         Print version
```

## Features

- **Live reload** — images appear in the browser the moment they land on disk, via WebSocket
- **Recursive watching** — monitors the entire directory tree, including newly created subdirectories
- **Responsive grid** — auto-sizing thumbnails with lazy loading
- **Lightbox** — click any image to view full-resolution
- **Pagination** — loads 15 images at a time with a "load more" button
- **Smart deny list** — skips `node_modules`, `.git`, `target`, `__pycache__`, and 16 other common directories by default
- **Debounced events** — 100ms debounce window prevents duplicate updates from editors and multi-stage writes
- **Path traversal protection** — image serving validates all paths stay within the watched root
- **Reconnection** — WebSocket auto-reconnects with exponential backoff (1s to 30s)
- **Zero config** — sensible defaults, just run `watchpix` and go

## Supported image formats

png, jpg, jpeg, gif, webp, svg, bmp, tiff, tif, ico, avif

## How it works

`watchpix` is a single Rust binary with no external runtime dependencies. On startup it:

1. Recursively scans the root directory for image files
2. Starts a filesystem watcher (inotify on Linux, FSEvents on macOS)
3. Serves a web gallery over HTTP with a WebSocket for live updates

When an image is created, modified, or deleted, the change propagates to all connected browsers within milliseconds.

## License

MIT
