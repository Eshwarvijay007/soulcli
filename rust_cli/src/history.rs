// Command history management
use std::{fs, path::PathBuf};

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
        if item.trim().is_empty() { return; }
        self.items.insert(0, item);
        if self.items.len() > self.cap {
            self.items.truncate(self.cap);
        }
        self.save();
    }

    pub fn clear(&mut self) {
        self.items.clear();
        self.save();
    }

    pub fn save(&self) {
        // Atomic-ish save: write to tmp then rename
        let tmp = self.path.with_extension("tmp");
        if let Err(e) = fs::write(&tmp, self.items.join("
")) {
            eprintln!("history save error (tmp write): {e}");
            return;
        }
        if let Err(e) = fs::rename(&tmp, &self.path) {
            let _ = fs::remove_file(&tmp);
            eprintln!("history save error (rename): {e}");
        }
    }
}
