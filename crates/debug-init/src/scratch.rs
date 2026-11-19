use std::env;
use std::fs;
use std::io;
use std::path::PathBuf;
use std::process;

pub struct Scratch(pub PathBuf);

impl Scratch {
    pub fn new(name: &str) -> io::Result<Self> {
        let path = env::temp_dir().join(format!("debug-init-{name}-{}", process::id()));
        if path.exists() {
            fs::remove_dir_all(&path)?;
        }
        fs::create_dir_all(&path)?;
        Ok(Self(path))
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
