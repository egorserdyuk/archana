use std::io::{Read, Seek};

pub trait ReadSeek: Read + Seek + Send {}

impl<T: Read + Seek + Send> ReadSeek for T {}

pub fn detect_magic(path: &std::path::Path, bytes: usize) -> std::io::Result<Vec<u8>> {
    let mut file = std::fs::File::open(path)?;
    let mut magic = vec![0u8; bytes];
    use std::io::Read;
    let n = file.read(&mut magic)?;
    magic.truncate(n);
    Ok(magic)
}

pub fn detect_format(path: &std::path::Path) -> std::io::Result<Option<crate::Format>> {
    let magic = detect_magic(path, 32)?;
    let registry = crate::registry::FormatRegistry::new();
    Ok(registry.detect(&magic))
}