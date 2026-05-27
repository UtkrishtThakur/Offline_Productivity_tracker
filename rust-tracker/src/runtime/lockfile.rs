use fs2::FileExt;
use std::fs::{File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};

pub struct TrackerLock {
    _file: File,
    #[allow(dead_code)]
    pub path: PathBuf,
}

impl TrackerLock {
    pub fn acquire<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let path = path.as_ref().to_path_buf();

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(false)
            .open(&path)?;

        file.try_lock_exclusive()?;

        Ok(Self {
            _file: file,
            path,
        })
    }
}