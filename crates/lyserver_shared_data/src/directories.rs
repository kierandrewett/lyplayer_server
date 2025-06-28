use std::{path::{Path, PathBuf}};

use crate::LYServerSharedData;

pub trait LYServerSharedDataDirectories {
    fn resolve_data_path(&self, path: &Path) -> PathBuf;
    fn resolve_data_path_str(&self, path: &'static str) -> PathBuf;
}

impl LYServerSharedDataDirectories for LYServerSharedData {
    fn resolve_data_path(&self, path: &Path) -> PathBuf {
        let mut data_path = self.data_dir.clone();
        data_path.push(path);

        return if let Err(_) = data_path.canonicalize() {
            data_path
        } else {
            data_path.canonicalize().unwrap()
        };
    }
    
    fn resolve_data_path_str(&self, path: &'static str) -> PathBuf {
        let mut data_path = self.data_dir.clone();
        data_path.push(Path::new(path));

        return if let Err(_) = data_path.canonicalize() {
            data_path
        } else {
            data_path.canonicalize().unwrap()
        };
    }
}