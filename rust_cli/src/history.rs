// Command history management will go here
use std::{fs, path::PathBuf};
//use directories::ProjectDirs;

pub struct History {
    path: PathBuf,
    pub items: Vec<String>,
    cap: usize,
}

impl History {
    pub fn new(path: PathBuf, cap: usize) -> Self {
        let items = fs::read_to_string(&path)
            .map(|c| c.lines().map(|s| s.to_string()).collect())
            .unwrap_or_default();
        
        Self { path, items, cap }
    }

    pub fn push(&mut self, item: String) {
        self.items.insert(0, item);
        if self.items.len() > self.cap {
            self.items.pop();
        }
        self.save();
    }

    pub fn save(&self) {
        let _ = fs::write(&self.path, self.items.join("\n"));
    }
}
