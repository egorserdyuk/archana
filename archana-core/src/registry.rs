use std::path::Path;

use crate::io::WriteSeek;
use crate::{AppendOptions, CreateOptions, Entry, ExtractOptions, Format, Result, Stats};

pub trait ArchiveReader: Send {
    fn entries(&mut self) -> Result<Box<dyn Iterator<Item = Result<Entry>> + '_>>;
    fn entry_by_name(&mut self, name: &str) -> Result<Option<Entry>>;
    fn extract_entry(&mut self, entry: &Entry, dest: &Path) -> Result<Stats>;
    fn extract_all(&mut self, dest: &Path, opts: &ExtractOptions) -> Result<Stats>;
}

pub trait ArchiveWriter: Send {
    fn append_file(&mut self, src: &Path, opts: &AppendOptions) -> Result<()>;
    fn append_dir_all(&mut self, src: &Path, opts: &AppendOptions) -> Result<()>;
    fn finish(self: Box<Self>) -> Result<Stats>;
}

pub(crate) trait FormatDriver: Send + Sync {
    fn magic_matches(&self, header: &[u8]) -> bool;
    fn format(&self) -> Format;
    fn open_read(&self, src: Box<dyn crate::io::ReadSeek>) -> Result<Box<dyn ArchiveReader>>;
    fn open_write(&self, dst: Box<dyn WriteSeek>, opts: &CreateOptions) -> Result<Box<dyn ArchiveWriter>>;
}

pub struct FormatRegistry {
    drivers: Vec<Box<dyn FormatDriver>>,
}

impl FormatRegistry {
    pub fn new() -> Self {
        Self { drivers: Vec::new() }
    }

    pub fn with_defaults() -> Self {
        Self::new()
            .with_driver(Box::new(crate::drivers::zip::ZipDriver))
    }

    pub fn with_driver(mut self, driver: Box<dyn FormatDriver>) -> Self {
        self.drivers.push(driver);
        self
    }

    pub fn detect(&self, src: &[u8]) -> Option<Format> {
        for driver in &self.drivers {
            if driver.magic_matches(src) {
                return Some(driver.format());
            }
        }
        None
    }

    pub fn driver(&self, format: Format) -> Option<&dyn FormatDriver> {
        for driver in &self.drivers {
            if driver.format() == format {
                return Some(driver.as_ref());
            }
        }
        None
    }

    pub fn has_format(&self, format: Format) -> bool {
        self.driver(format).is_some()
    }
}