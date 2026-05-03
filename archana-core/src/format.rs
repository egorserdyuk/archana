#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Zip,
    Jar,
    Rar,
    SevenZip,
    Tar,
    TarGz,
    TarBz2,
    TarXz,
    TarZst,
}

impl Format {
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_lowercase().as_str() {
            "zip" | "jar" => Some(Format::Zip),
            "rar" => Some(Format::Rar),
            "7z" => Some(Format::SevenZip),
            "tar" => Some(Format::Tar),
            "tar.gz" | "tgz" => Some(Format::TarGz),
            "tar.bz2" | "tbz2" => Some(Format::TarBz2),
            "tar.xz" | "txz" => Some(Format::TarXz),
            "tar.zst" | "tzst" => Some(Format::TarZst),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Compression {
    #[default]
    Store,
    Deflate,
    Lzma,
    Bzip2,
    Zstd,
    Xz,
}