use std::io::{Read, Write};
use std::path::Path;
use zip::ZipArchive;

use crate::error::Error;
use crate::format::Compression;
use crate::io::{ReadSeek, WriteSeek};
use crate::options::{AppendOptions, CreateOptions, ExtractOptions};
use crate::registry::{ArchiveReader, ArchiveWriter};
use crate::{Entry, EntryKind, Result, Stats};

pub struct ZipDriver;

impl crate::registry::FormatDriver for ZipDriver {
    fn magic_matches(&self, header: &[u8]) -> bool {
        header.len() >= 4 && &header[0..4] == b"PK\x03\x04"
    }

    fn format(&self) -> crate::format::Format {
        crate::format::Format::Zip
    }

    fn open_read(&self, src: Box<dyn ReadSeek>) -> Result<Box<dyn ArchiveReader>> {
        Ok(Box::new(ZipReader::new(src)?))
    }

    fn open_write(&self, dst: Box<dyn WriteSeek>, opts: &CreateOptions) -> Result<Box<dyn ArchiveWriter>> {
        Ok(Box::new(ZipWriter::new(dst, opts)?))
    }
}

struct ZipReaderInner {
    archive: ZipArchive<Box<dyn ReadSeek>>,
}

pub struct ZipReader {
    inner: Option<ZipReaderInner>,
}

impl ZipReader {
    pub fn new(src: Box<dyn ReadSeek>) -> Result<Self> {
        let archive = ZipArchive::new(src).map_err(|e| Error::Custom(e.to_string()))?;
        Ok(Self {
            inner: Some(ZipReaderInner { archive }),
        })
    }
}

impl ArchiveReader for ZipReader {
    fn entries(&mut self) -> Result<Box<dyn Iterator<Item = Result<Entry>> + '_>> {
        let inner = self.inner.as_mut().ok_or_else(|| Error::Custom("Archive not open".into()))?;
        let entries: Vec<Entry> = (0..inner.archive.len())
            .filter_map(|i| {
                let file = inner.archive.by_index(i).ok()?;
                let compression = match file.compression() {
                    zip::CompressionMethod::Stored => Compression::Store,
                    zip::CompressionMethod::Deflated => Compression::Deflate,
                    _ => Compression::Deflate,
                };
                Some(Entry {
                    name: file.name().to_string(),
                    size_compressed: file.compressed_size() as u64,
                    size_uncompressed: file.size() as u64,
                    modified: file.last_modified().and_then(|t| {
                        std::time::SystemTime::from(std::time::UNIX_EPOCH)
                            .checked_add(std::time::Duration::from_secs(
                                (t.year() as u64 - 1970) * 31536000
                                + (t.month() as u64) * 2592000
                                + (t.day() as u64) * 86400
                                + (t.hour() as u64) * 3600
                                + (t.minute() as u64) * 60
                                + t.second() as u64,
                            ))
                    }),
                    kind: if file.is_dir() { EntryKind::Directory } else { EntryKind::File },
                    compression,
                })
            })
            .collect();
        Ok(Box::new(entries.into_iter().map(Ok)))
    }

    fn entry_by_name(&mut self, name: &str) -> Result<Option<Entry>> {
        let inner = self.inner.as_mut().ok_or_else(|| Error::Custom("Archive not open".into()))?;
        if let Ok(file) = inner.archive.by_name(name) {
            let compression = match file.compression() {
                zip::CompressionMethod::Stored => Compression::Store,
                zip::CompressionMethod::Deflated => Compression::Deflate,
                _ => Compression::Deflate,
            };
            Ok(Some(Entry {
                name: file.name().to_string(),
                size_compressed: file.compressed_size() as u64,
                size_uncompressed: file.size() as u64,
                modified: None,
                kind: if file.is_dir() { EntryKind::Directory } else { EntryKind::File },
                compression,
            }))
        } else {
            Ok(None)
        }
    }

    fn extract_entry(&mut self, entry: &Entry, dest: &Path) -> Result<Stats> {
        let inner = self.inner.as_mut().ok_or_else(|| Error::Custom("Archive not open".into()))?;
        
        let mut file = inner.archive.by_name(&entry.name)
            .map_err(|_| Error::EntryNotFound(entry.name.clone()))?;
        
        let out_path = dest.join(&entry.name);
        if entry.kind == EntryKind::Directory {
            std::fs::create_dir_all(&out_path)?;
            return Ok(Stats::default());
        }
        
        if let Some(parent) = out_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        
        let mut out_file = std::fs::File::create(&out_path)?;
        std::io::copy(&mut file, &mut out_file)?;
        
        Ok(Stats {
            files: 1,
            bytes_in: entry.size_uncompressed,
            bytes_out: entry.size_compressed,
            elapsed_ms: 0,
        })
    }

    fn extract_all(&mut self, dest: &Path, _opts: &ExtractOptions) -> Result<Stats> {
        let inner = self.inner.as_mut().ok_or_else(|| Error::Custom("Archive not open".into()))?;
        
        let mut total = Stats::default();
        for i in 0..inner.archive.len() {
            let name = {
                let file = inner.archive.by_index(i).map_err(|e| Error::Custom(e.to_string()))?;
                file.name().to_string()
            };
            let is_dir = name.ends_with('/');
            
            if is_dir {
                let path = dest.join(&name);
                std::fs::create_dir_all(&path)?;
                total.files += 1;
            } else {
                let path = dest.join(&name);
                if let Some(parent) = path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                let mut out = std::fs::File::create(&path)?;
                let mut reader = inner.archive.by_index(i).map_err(|e| Error::Custom(e.to_string()))?;
                std::io::copy(&mut reader, &mut out)?;
                total.files += 1;
            }
        }
        Ok(total)
    }
}

pub struct ZipWriter {
    writer: Option<zip::ZipWriter<Box<dyn WriteSeek>>>,
}

impl ZipWriter {
    pub fn new(dst: Box<dyn WriteSeek>, _opts: &CreateOptions) -> Result<Self> {
        let writer = zip::ZipWriter::new(dst);
        Ok(Self {
            writer: Some(writer),
        })
    }
}

impl ArchiveWriter for ZipWriter {
    fn append_file(&mut self, src: &Path, opts: &AppendOptions) -> Result<()> {
        let writer = self.writer.as_mut().ok_or_else(|| Error::Custom("Writer not open".into()))?;
        
        let name = src.file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| Error::custom("Invalid filename"))?;
        
        let mut reader = std::fs::File::open(src)?;
        let mut buffer = Vec::new();
        reader.read_to_end(&mut buffer)?;
        
        let compression = match opts.compression.unwrap_or(Compression::Deflate) {
            Compression::Store => zip::CompressionMethod::Stored,
            Compression::Deflate => zip::CompressionMethod::Deflated,
            _ => zip::CompressionMethod::Deflated,
        };
        
        writer.start_file(name, zip::write::SimpleFileOptions::default()
            .compression_method(compression))
            .map_err(|e| Error::Custom(e.to_string()))?;
        
        writer.write_all(&buffer).map_err(|e| Error::Custom(e.to_string()))?;
        
        Ok(())
    }

    fn append_dir_all(&mut self, src: &Path, opts: &AppendOptions) -> Result<()> {
        let writer = self.writer.as_mut().ok_or_else(|| Error::Custom("Writer not open".into()))?;
        
        for entry in walkdir::WalkDir::new(src) {
            let entry = entry.map_err(|e| Error::Custom(e.to_string()))?;
            let path = entry.path();
            let name = path.strip_prefix(src)
                .map_err(|e| Error::Custom(e.to_string()))?
                .to_string_lossy()
                .replace('\\', "/");
            
            if path.is_dir() {
                if !name.is_empty() {
                    writer.add_directory(&name, zip::write::SimpleFileOptions::default())
                        .map_err(|e| Error::Custom(e.to_string()))?;
                }
            } else {
                let mut reader = std::fs::File::open(path)?;
                let mut buffer = Vec::new();
                reader.read_to_end(&mut buffer)?;
                
                let compression = match opts.compression.unwrap_or(Compression::Deflate) {
                    Compression::Store => zip::CompressionMethod::Stored,
                    Compression::Deflate => zip::CompressionMethod::Deflated,
                    _ => zip::CompressionMethod::Deflated,
                };
                
                writer.start_file(&name, zip::write::SimpleFileOptions::default()
                    .compression_method(compression))
                    .map_err(|e| Error::Custom(e.to_string()))?;
                
                writer.write_all(&buffer).map_err(|e| Error::Custom(e.to_string()))?;
            }
        }
        Ok(())
    }

    fn finish(self: Box<Self>) -> Result<Stats> {
        let mut writer = self.writer.ok_or_else(|| Error::Custom("Writer not open".into()))?;
        writer.finish().map_err(|e| Error::Custom(e.to_string()))?;
        Ok(Stats {
            files: 0,
            bytes_in: 0,
            bytes_out: 0,
            elapsed_ms: 0,
        })
    }
}