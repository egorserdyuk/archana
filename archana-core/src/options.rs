use crate::Compression;

pub struct OpenOptions {
    password: Option<String>,
}

impl OpenOptions {
    pub fn new() -> Self {
        Self { password: None }
    }
    
    pub fn password(mut self, password: impl Into<String>) -> Self {
        self.password = Some(password.into());
        self
    }
    
    pub fn password_ref(&self) -> Option<&str> {
        self.password.as_deref()
    }
}

impl Default for OpenOptions {
    fn default() -> Self {
        Self::new()
    }
}

pub struct CreateOptions {
    compression: Option<Compression>,
    password: Option<String>,
}

impl CreateOptions {
    pub fn new() -> Self {
        Self {
            compression: None,
            password: None,
        }
    }
    
    pub fn compression(mut self, compression: Compression) -> Self {
        self.compression = Some(compression);
        self
    }
    
    pub fn password(mut self, password: impl Into<String>) -> Self {
        self.password = Some(password.into());
        self
    }
    
    pub fn get_compression(&self) -> Option<Compression> {
        self.compression
    }
    
    pub fn password_ref(&self) -> Option<&str> {
        self.password.as_deref()
    }
}

impl Default for CreateOptions {
    fn default() -> Self {
        Self::new()
    }
}

pub struct ExtractOptions {
    password: Option<String>,
    overwrite: bool,
}

impl ExtractOptions {
    pub fn new() -> Self {
        Self {
            password: None,
            overwrite: false,
        }
    }
    
    pub fn password(mut self, password: impl Into<String>) -> Self {
        self.password = Some(password.into());
        self
    }
    
    pub fn overwrite(mut self, overwrite: bool) -> Self {
        self.overwrite = overwrite;
        self
    }
    
    pub fn password_ref(&self) -> Option<&str> {
        self.password.as_deref()
    }
    
    pub fn is_overwrite(&self) -> bool {
        self.overwrite
    }
}

impl Default for ExtractOptions {
    fn default() -> Self {
        Self::new()
    }
}

pub struct AppendOptions {
    pub compression: Option<Compression>,
}

impl AppendOptions {
    pub fn new() -> Self {
        Self { compression: None }
    }
    
    pub fn compression(mut self, compression: Compression) -> Self {
        self.compression = Some(compression);
        self
    }
}

impl Default for AppendOptions {
    fn default() -> Self {
        Self::new()
    }
}