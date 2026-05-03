pub mod archive;
pub mod drivers;
pub mod error;
pub mod format;
pub mod io;
pub mod options;
pub mod registry;

pub use archive::{Archive, Entry, EntryKind, Stats};
pub use error::Error;
pub use format::{Compression, Format};
pub use options::{AppendOptions, CreateOptions, ExtractOptions, OpenOptions};
pub use registry::FormatRegistry;

pub type Result<T> = std::result::Result<T, Error>;