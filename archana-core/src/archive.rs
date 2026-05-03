use std::path::Path;

use crate::error::Error;
use crate::format::{Compression, Format};
use crate::io;
use crate::options::{AppendOptions, CreateOptions, ExtractOptions, OpenOptions};
use crate::registry::{FormatRegistry, ArchiveReader};
use crate::Result;

pub struct Archive {
    reader: Option<Box<dyn ArchiveReader>>,
    writer: Option<Box<dyn crate::registry::ArchiveWriter>>,
    format: Format,
}

impl Archive {
    pub fn open(path: impl AsRef<Path>, _options: OpenOptions) -> Result<Self> {
        let path = path.as_ref();
        
        let magic = io::detect_magic(path, 32)
            .map_err(|e| Error::Io(e))?;
        
        let format = FormatRegistry::with_defaults()
            .detect(&magic)
            .ok_or_else(|| Error::UnsupportedFormat(
                path.extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("unknown")
                    .to_string()
            ))?;
        
        let file = std::fs::File::open(path)?;
        let reader = Self::create_reader(format, Box::new(file))?;
        
        Ok(Self {
            reader: Some(reader),
            writer: None,
            format,
        })
    }

    pub fn create(path: impl AsRef<Path>, format: Format, options: CreateOptions) -> Result<Self> {
        let file = std::fs::File::create(path.as_ref())?;
        let writer = Self::create_writer(format, Box::new(file), &options)?;
        
        Ok(Self {
            reader: None,
            writer: Some(writer),
            format,
        })
    }

    fn create_reader(format: Format, src: Box<dyn io::ReadSeek>) -> Result<Box<dyn ArchiveReader>> {
        match format {
            Format::Zip | Format::Jar => {
                Ok(Box::new(crate::drivers::zip::ZipReader::new(src)?) as Box<dyn ArchiveReader>)
            }
            _ => Err(Error::UnsupportedFormat(format!("{:?}", format))),
        }
    }

    fn create_writer(format: Format, dst: Box<dyn crate::io::WriteSeek>, opts: &CreateOptions) -> Result<Box<dyn crate::registry::ArchiveWriter>> {
        match format {
            Format::Zip | Format::Jar => {
                Ok(Box::new(crate::drivers::zip::ZipWriter::new(dst, opts)?) as Box<dyn crate::registry::ArchiveWriter>)
            }
            _ => Err(Error::UnsupportedFormat(format!("{:?}", format))),
        }
    }

    pub fn entries(&mut self) -> Result<Box<dyn Iterator<Item = Result<Entry>> + '_>> {
        match &mut self.reader {
            Some(r) => r.entries(),
            None => Err(Error::Custom("Archive not open for reading".into())),
        }
    }

    pub fn entry_by_name(&mut self, name: &str) -> Result<Option<Entry>> {
        match &mut self.reader {
            Some(r) => r.entry_by_name(name),
            None => Err(Error::Custom("Archive not open for reading".into())),
        }
    }

    pub fn extract_all(&mut self, dest: impl AsRef<Path>, opts: ExtractOptions) -> Result<Stats> {
        match &mut self.reader {
            Some(r) => r.extract_all(dest.as_ref(), &opts),
            None => Err(Error::Custom("Archive not open for reading".into())),
        }
    }

    pub fn extract_entry(&mut self, entry: &Entry, dest: impl AsRef<Path>) -> Result<Stats> {
        match &mut self.reader {
            Some(r) => r.extract_entry(entry, dest.as_ref()),
            None => Err(Error::Custom("Archive not open for reading".into())),
        }
    }

    pub fn append_file(&mut self, src: impl AsRef<Path>, opts: AppendOptions) -> Result<()> {
        match &mut self.writer {
            Some(w) => w.append_file(src.as_ref(), &opts),
            None => Err(Error::Custom("Archive not open for writing".into())),
        }
    }

    pub fn append_dir_all(&mut self, src: impl AsRef<Path>, opts: AppendOptions) -> Result<()> {
        match &mut self.writer {
            Some(w) => w.append_dir_all(src.as_ref(), &opts),
            None => Err(Error::Custom("Archive not open for writing".into())),
        }
    }

    pub fn finish(mut self) -> Result<Stats> {
        match self.writer.take() {
            Some(w) => w.finish(),
            None => Err(Error::Custom("Archive not open for writing".into())),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Entry {
    pub name: String,
    pub size_compressed: u64,
    pub size_uncompressed: u64,
    pub modified: Option<std::time::SystemTime>,
    pub kind: EntryKind,
    pub compression: Compression,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EntryKind {
    #[default]
    File,
    Directory,
    Symlink,
}

#[derive(Debug, Clone, Default)]
pub struct Stats {
    pub files: u64,
    pub bytes_in: u64,
    pub bytes_out: u64,
    pub elapsed_ms: u64,
}

impl std::ops::AddAssign for Stats {
    fn add_assign(&mut self, other: Self) {
        self.files += other.files;
        self.bytes_in += other.bytes_in;
        self.bytes_out += other.bytes_out;
    }
}