# ARCHITECTURE.md — archana

> A lightweight, OS-agnostic archive manager written in Rust.  
> Supports `.zip`, `.jar`, `.rar`, `.7z`, `.tar`, `.tar.gz`, `.tar.bz2`, `.tar.xz` and more.  
> Dual-mode: blazing-fast CLI and a minimal, native GUI — both sharing a single core library.

---

## Table of Contents

1. [Design Philosophy](#1-design-philosophy)
2. [High-Level Structure](#2-high-level-structure)
3. [Crate Layout](#3-crate-layout)
4. [Core Library (`archana-core`)](#4-core-library-archana-core)
5. [Format Drivers](#5-format-drivers)
6. [CLI Frontend (`archana-cli`)](#6-cli-frontend-archana-cli)
7. [GUI Frontend (`archana-gui`)](#7-gui-frontend-archana-gui)
8. [Threading & Async Model](#8-threading--async-model)
9. [Error Handling](#9-error-handling)
10. [Dependency Policy](#10-dependency-policy)
11. [Platform Portability](#11-platform-portability)
12. [Performance Strategies](#12-performance-strategies)
13. [Testing Strategy](#13-testing-strategy)
14. [Directory Tree](#14-directory-tree)
15. [Data Flow Diagrams](#15-data-flow-diagrams)

---

## 1. Design Philosophy

| Principle | How archana applies it |
|---|---|
| **Zero bloat** | Every dependency earns its place — pure-Rust crates are strongly preferred over C bindings |
| **Single source of truth** | One core library, two thin frontends; no logic duplication |
| **Fail loudly** | Errors propagate with full context; no silent data loss |
| **Stream-first** | Files are never fully read into RAM unless the format strictly requires it |
| **Compile-time safety** | Format dispatch uses sealed traits + enums, not `dyn Any` or stringly-typed branches |
| **Minimal syscalls** | Bulk I/O uses `BufReader`/`BufWriter` with tuned buffer sizes; `sendfile(2)` / `CopyFileRange` on Linux |

---

## 2. High-Level Structure

```
┌──────────────────────────────────────────────────────────┐
│                        User                              │
└─────────────┬────────────────────────┬───────────────────┘
              │  CLI args              │  GUI events
     ┌────────▼──────────┐   ┌─────────▼──────────┐
     │   archana-cli     │   │   archana-gui       │
     │  (clap, thin)     │   │  (egui / iced)      │
     └────────┬──────────┘   └─────────┬──────────┘
              │                        │
              └──────────┬─────────────┘
                         │  Public API  (archana-core)
              ┌──────────▼──────────────────────┐
              │          archana-core            │
              │  ┌────────────────────────────┐  │
              │  │      ArchiveManager        │  │
              │  └──────────────┬─────────────┘  │
              │  ┌──────────────▼─────────────┐  │
              │  │      Format Registry       │  │
              │  └──┬───────┬───────┬─────────┘  │
              │  ┌──▼─┐  ┌──▼─┐  ┌──▼──┐         │
              │  │ZIP │  │RAR │  │ 7Z  │  …       │
              │  │drv │  │drv │  │ drv │          │
              │  └────┘  └────┘  └─────┘          │
              │  ┌─────────────────────────────┐  │
              │  │   I/O Streams / FS helpers  │  │
              │  └─────────────────────────────┘  │
              └─────────────────────────────────── ┘
```

---

## 3. Crate Layout

archana is structured as a **Cargo workspace** with three member crates:

```
archana/
├── Cargo.toml          ← workspace manifest
├── archana-core/       ← format-agnostic engine (library)
├── archana-cli/        ← CLI binary (thin wrapper)
└── archana-gui/        ← GUI binary (thin wrapper)
```

Building only what you need:

```sh
cargo build -p archana-cli --release          # CLI only
cargo build -p archana-gui --release          # GUI only
cargo build --release                         # both
```

Feature flags on `archana-core` gate optional format support so users who only need ZIP+TAR pay zero cost for RAR/7z codecs.

---

## 4. Core Library (`archana-core`)

### 4.1 Public API surface

```rust
/// Opaque handle to an open archive (read or write).
pub struct Archive { /* private */ }

impl Archive {
    pub fn open(path: impl AsRef<Path>, options: OpenOptions) -> Result<Self>;
    pub fn create(path: impl AsRef<Path>, format: Format, options: CreateOptions) -> Result<Self>;

    // Inspection
    pub fn entries(&self) -> Result<impl Iterator<Item = Result<Entry>> + '_>;
    pub fn entry_by_name(&self, name: &str) -> Result<Option<Entry>>;

    // Extraction
    pub fn extract_all(&self, dest: impl AsRef<Path>, opts: ExtractOptions) -> Result<Stats>;
    pub fn extract_entry(&self, entry: &Entry, dest: impl AsRef<Path>) -> Result<Stats>;

    // Compression
    pub fn append_file(&mut self, src: impl AsRef<Path>, opts: AppendOptions) -> Result<()>;
    pub fn append_dir_all(&mut self, src: impl AsRef<Path>, opts: AppendOptions) -> Result<()>;
    pub fn finish(self) -> Result<Stats>;
}

/// Metadata about a single archive entry (file/dir/symlink).
pub struct Entry {
    pub name: String,
    pub size_compressed: u64,
    pub size_uncompressed: u64,
    pub modified: Option<SystemTime>,
    pub kind: EntryKind,             // File | Directory | Symlink
    pub compression: Compression,    // Store | Deflate | Lzma | Bzip2 | Zstd | …
}
```

### 4.2 Format detection

Detection is done in priority order — **never trust the file extension alone**:

1. **Magic bytes** — first 8–12 bytes of the stream (fastest, most reliable).
2. **File extension** — used as a hint when magic is ambiguous (e.g., `.jar` vs `.zip`).
3. **Explicit override** — caller can force a `Format` variant.

```rust
pub enum Format {
    Zip,
    Jar,     // ZIP dialect with MANIFEST.MF semantics
    Rar,     // RAR4 + RAR5
    SevenZip,
    Tar,
    TarGz,
    TarBz2,
    TarXz,
    TarZst,
}
```

### 4.3 Format Registry

Each format implements the `FormatDriver` sealed trait:

```rust
// Sealed — only archana-core can implement this.
pub(crate) trait FormatDriver: Send + Sync {
    fn magic_matches(header: &[u8]) -> bool where Self: Sized;
    fn open_read(&self, src: Box<dyn ReadSeek>) -> Result<Box<dyn ArchiveReader>>;
    fn open_write(&self, dst: Box<dyn Write>, opts: &CreateOptions) -> Result<Box<dyn ArchiveWriter>>;
}
```

`FormatRegistry` is a compile-time array of `&'static dyn FormatDriver` — no heap allocation, no `HashMap`, O(n) magic scan over ≤10 entries.

---

## 5. Format Drivers

| Format | Rust crate | Notes |
|---|---|---|
| ZIP / JAR | `zip` (pure Rust) | Deflate, Store, Bzip2, Zstd level support |
| TAR family | `tar` + `flate2` / `bzip2` / `xz2` / `zstd` | Stream-only, no seek required |
| RAR | `unrar` (libunrar FFI) **or** `rar` pure-Rust crate | FFI behind `feature = "rar"` flag; pure-Rust preferred when available |
| 7-Zip | `sevenz-rust` (pure Rust) | LZMA2 decompression + basic write |

All drivers expose only `Read + Write` streams to the core — they never touch the filesystem directly. This makes them trivially testable with in-memory buffers.

### Compression codec selection

Write path picks a codec based on:

```
user hint → format default → file-type heuristic
```

Large already-compressed inputs (`.jpg`, `.mp4`, `.png` …) are stored uncompressed (`Store`) automatically unless the user forces otherwise.

---

## 6. CLI Frontend (`archana-cli`)

### Design goals
- **Zero startup overhead** — no async runtime, no global state.
- **POSIX-compatible** exit codes (`0` success, `1` error, `2` usage error).
- **`tar`-like UX** familiar to power users; short aliases for all common ops.

### Argument structure (`clap` derive)

```
archana <COMMAND> [OPTIONS] <ARCHIVE> [FILES…]

Commands:
  a, add        Add files to an archive
  x, extract    Extract archive contents
  l, list       List archive entries
  t, test       Test archive integrity
  d, delete     Delete entries from an archive
  i, info       Show archive metadata

Global options:
  -f, --format <FORMAT>    Force archive format
  -j, --threads <N>        Worker threads (default: logical CPU count)
  -v, --verbose            Verbose output
  -q, --quiet              Suppress progress output
      --password <PASS>    Encryption password
```

### Progress rendering

A lightweight custom progress bar writes directly to `stderr` using ANSI escape codes — no external crate. It degrades gracefully to plain line output when `stderr` is not a TTY (`isatty(2)` check on startup).

---

## 7. GUI Frontend (`archana-gui`)

### Toolkit choice

**[egui](https://github.com/emilk/egui)** via `eframe` — immediate-mode, pure Rust, ships its own renderer (wgpu or glow), cross-platform with a single binary. No system GTK/Qt/Cocoa dependency.

Alternative: **[iced](https://github.com/iced-rs/iced)** (Elm-architecture, also pure Rust). The GUI crate isolates toolkit choice behind a thin `UiBackend` trait so it can be swapped without touching core.

### GUI panels

```
┌──────────────────────────────────────────────────────┐
│  Menu: File | Edit | View | Help          [─][□][✕] │
├──────────────────────────────────────────────────────┤
│  Toolbar: [Open] [New] [Extract] [Add] [Delete]      │
├─────────────────────────────────────────────────────┤
│  Path bar:  📁 my-archive.zip / subfolder/           │
├──────────────────────────────────────────────────────┤
│  Entry list (virtual scroll, sorted columns)         │
│  Name       │ Size │ Packed │ Modified │ Type        │
│  ──────────────────────────────────────────────────  │
│  file.txt   │ 4 KB │ 1 KB   │ 2024-…   │ Deflate     │
│  image.png  │ 2 MB │ 2 MB   │ 2024-…   │ Store       │
├──────────────────────────────────────────────────────┤
│  Status bar:  3 files selected │ 6.1 MB  │ Ready     │
└──────────────────────────────────────────────────────┘
```

### GUI ↔ Core communication

The GUI runs `archana-core` operations on a **dedicated worker thread pool** and receives progress via an `mpsc` channel. The UI thread never blocks on I/O.

```
UI thread                    Worker thread
   │──── Command::Extract ──────►│
   │                             │  archana-core::Archive::extract_all(…)
   │◄─── Progress { pct, … } ───│
   │◄─── Progress { pct, … } ───│
   │◄─── Done(Stats) ───────────│
```

---

## 8. Threading & Async Model

archana is **synchronous + thread-pool based** — no `tokio`/`async-std` runtime. Rationale: archive I/O is CPU-bound (compression) or sequential-I/O-bound, not concurrent-network-bound. A lightweight thread pool avoids async overhead and keeps the binary small.

```
┌─────────────────────────────────┐
│       ThreadPool (rayon)        │
│  ┌──────────┐  ┌──────────┐    │
│  │ Worker 0 │  │ Worker 1 │ …  │
│  │ compress │  │ compress │    │
│  │ entry A  │  │ entry B  │    │
│  └──────────┘  └──────────┘    │
└─────────────────────────────────┘
        ↓ results via channel ↓
     Main / UI thread (collect)
```

**Parallel compression** (add path):
- Independent entries are compressed concurrently with `rayon::par_iter`.
- Central directory / index is written serially after all workers finish.

**Parallel extraction** (extract path):
- Entries with no inter-dependency are extracted concurrently.
- Symlinks and directory creation are serialised on the main thread to avoid race conditions.

---

## 9. Error Handling

All public API functions return `archana_core::Result<T>`, a type alias for `std::result::Result<T, archana_core::Error>`.

```rust
#[non_exhaustive]
pub enum Error {
    Io(std::io::Error),
    UnsupportedFormat(String),
    CorruptArchive { entry: String, reason: String },
    PasswordRequired,
    WrongPassword,
    EntryNotFound(String),
    PermissionDenied(PathBuf),
    PathTraversal(String),    // Zip-slip protection
    Custom(String),
}
```

**Zip-slip protection** is enforced at the core level: every extracted path is canonically resolved and checked to reside inside the destination directory before any bytes are written.

---

## 10. Dependency Policy

### Tier 1 — Always allowed (zero-cost abstractions)
- `std` only constructs

### Tier 2 — Allowed (pure Rust, well-audited)
- `zip`, `tar`, `flate2`, `bzip2`, `xz2`, `zstd`, `sevenz-rust`
- `clap` (CLI only), `egui`/`eframe` (GUI only)
- `rayon` (parallel iterators)
- `thiserror` (error derive)

### Tier 3 — Feature-gated (C FFI, optional)
- `unrar` (libunrar bindings) — only when `features = ["rar-ffi"]`

### Explicitly banned
- Any async runtime (`tokio`, `async-std`) — unnecessary for this domain
- `serde` — not needed for archive data paths; only used if config file support is added
- Any crate with transitive dependency count > 30 without strong justification

Dependency count target: **< 25 direct + transitive crates** in the default feature set.

---

## 11. Platform Portability

| Concern | Strategy |
|---|---|
| Path separators | All internal paths use `/` (POSIX); conversion on Windows via `std::path` |
| File permissions | Stored/restored on Unix; mapped to read-only on Windows where applicable |
| Symlinks | Created on Unix; skipped with a warning on Windows (requires elevated rights) |
| Large files | `u64` sizes everywhere; `seek(SeekFrom::Start(u64))` — no `i32` offsets |
| Endianness | All archive formats define their own byte order; handled per-driver |
| Filename encoding | ZIP central directory decoded as UTF-8; fallback to CP437 (spec-compliant) |
| GUI rendering | egui/wgpu renders identically on Windows, macOS, Linux |
| CI matrix | GitHub Actions: `ubuntu-latest`, `windows-latest`, `macos-latest` |

---

## 12. Performance Strategies

### I/O
- `BufReader` / `BufWriter` with a **64 KB** buffer (tuned to page cache granularity).
- `std::fs::copy` / platform `copy_file_range` for store-mode extraction (zero-copy where OS supports it).
- Memory-mapped reads (`memmap2`) for random-access formats (7z, RAR) behind a feature flag.

### Compression
- Deflate: `flate2` with `miniz_oxide` backend (pure Rust, SIMD-accelerated on x86).
- Zstd: dictionary training automatically reused across small similar files.
- Compression level defaults: `6` (balanced). CLI `--fast` sets `1`, `--best` sets `19`.

### Allocation
- `Entry` metadata is stored in a flat `Vec<Entry>` — no tree allocations during listing.
- String interning for repeated directory prefixes in large archives.
- Stack-allocated small buffers (`[u8; 64]`) for magic-byte detection — no heap touch on format probe.

### Binary size
- `opt-level = "z"` + `lto = true` + `codegen-units = 1` in release profile.
- `strip = true` strips debug symbols in release.
- Target binary size goal: **CLI < 5 MB**, **GUI < 15 MB** (statically linked, release).

---

## 13. Testing Strategy

```
archana-core/tests/
├── unit/
│   ├── format_detection.rs     ← magic byte edge cases
│   ├── path_traversal.rs       ← zip-slip corpus
│   └── codec_roundtrip.rs      ← compress → decompress identity checks
├── integration/
│   ├── zip_roundtrip.rs
│   ├── tar_roundtrip.rs
│   ├── sevenzip_extract.rs
│   └── large_file.rs           ← files > 4 GB (ZIP64)
└── fixtures/
    ├── good/                   ← known-good archive samples
    └── malformed/              ← fuzz-generated corrupt inputs
```

- **Unit tests** run in-memory with `std::io::Cursor` — no filesystem access.
- **Integration tests** use `tempfile::TempDir` and are gated behind `#[cfg(test)]`.
- **Fuzz targets** (`cargo-fuzz`) cover the format detection and extraction paths.
- **Benchmarks** (`criterion`) track compression throughput (MB/s) and extraction latency per format.

---

## 14. Directory Tree

```
archana/
├── Cargo.toml                    ← workspace
├── docs/ARCHITECTURE.md
├── README.md
├── LICENSE
│
├── archana-core/
│   ├── Cargo.toml
│   ├── src/
│   │   ├── lib.rs                ← public API re-exports
│   │   ├── archive.rs            ← Archive, Entry, Stats structs
│   │   ├── error.rs              ← Error enum
│   │   ├── format.rs             ← Format enum + detection
│   │   ├── registry.rs           ← FormatRegistry
│   │   ├── options.rs            ← OpenOptions, CreateOptions, ExtractOptions
│   │   ├── io.rs                 ← ReadSeek, buffered helpers
│   │   └── drivers/
│   │       ├── mod.rs
│   │       ├── zip.rs
│   │       ├── tar.rs
│   │       ├── sevenzip.rs
│   │       └── rar.rs            ← feature-gated
│   └── tests/
│       ├── unit/
│       ├── integration/
│       └── fixtures/
│
├── archana-cli/
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs
│       ├── args.rs               ← clap derive structs
│       ├── commands/
│       │   ├── add.rs
│       │   ├── extract.rs
│       │   ├── list.rs
│       │   ├── test.rs
│       │   └── info.rs
│       └── progress.rs           ← ANSI progress bar
│
└── archana-gui/
    ├── Cargo.toml
    └── src/
        ├── main.rs
        ├── app.rs                ← eframe::App impl
        ├── worker.rs             ← background thread + channel
        ├── views/
        │   ├── entry_list.rs
        │   ├── toolbar.rs
        │   └── status_bar.rs
        └── icons/                ← embedded SVG icons (no external asset loader)
```

---

## 15. Data Flow Diagrams

### Extract flow

```
User input (path + dest)
        │
        ▼
  Format::detect(path)          ← magic bytes probe, O(1) read
        │
        ▼
  FormatRegistry::driver()      ← compile-time dispatch
        │
        ▼
  driver.open_read(BufReader)
        │
        ▼
  ┌─── entries.par_iter() ──────────────────────────┐
  │  for each Entry:                                │
  │    validate_path(entry.name, dest)  ← zip-slip  │
  │    decompress_stream → BufWriter(dest_file)      │
  │    set_mtime + permissions                       │
  └─────────────────────────────────────────────────┘
        │
        ▼
  Stats { files, bytes_in, bytes_out, elapsed }
        │
        ▼
  CLI: print summary / GUI: update status bar
```

### Add / compress flow

```
User input (src files + archive path + format)
        │
        ▼
  Archive::create(path, format, opts)
        │
        ▼
  src_files.par_iter()
    │  for each file:
    │    open BufReader(src)
    │    select codec (heuristic or user hint)
    │    compress → in-memory buffer  OR  temp file
    │    send (entry_meta, compressed_bytes) to collector
        │
        ▼ (serial)
  ArchiveWriter::write_entry(meta, data)   ← per-format
        │
        ▼
  ArchiveWriter::finish()   ← write central dir / end-of-archive
        │
        ▼
  Stats
```

---

*This document should be kept in sync with the workspace `Cargo.toml`. When adding a new format driver, update §5, §10, and §14.*
