// Autocorrection logic will go here
use std::{collections::HashMap, fs, path::PathBuf};
use directories::ProjectDirs;
use strsim::levenshtein;

pub struct AutoCorrect {
    pub map: HashMap<String, String>,
    pub path: PathBuf,
}

impl AutoCorrect {
    pub fn load() -> Self {
        let proj = ProjectDirs::from("com", "soulshell", "soulshell").unwrap();
        let path = proj.config_dir().join("autocorrect.json");
        fs::create_dir_all(proj.config_dir()).ok();
        let map = fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or(HashMap::new());
        Self { map, path }
    }

    pub fn save(&self) {
        let _ = fs::write(&self.path, serde_json::to_string_pretty(&self.map).unwrap());
    }

    pub fn learn(&mut self, wrong: &str, right: &str) {
        if wrong != right {
            self.map.insert(wrong.to_string(), right.to_string());
            self.save();
        }
    }

    pub fn correct_line(&self, line: &str) -> String {
        // Correct only the first token (command) and leave args untouched
        let mut parts = line.splitn(2, ' ');
        let cmd = parts.next().unwrap_or("");
        let rest = parts.next().unwrap_or("");
        let corrected = self.correct_token(cmd);
        if rest.is_empty() { corrected } else { format!("{} {}", corrected, rest) }
    }

    fn correct_token(&self, token: &str) -> String {
        if let Some(hit) = self.map.get(token) { return hit.clone(); }
        let known = [
            "git","npm","npx","node","python","pip","poetry","make",
            "docker","kubectl","cargo","rg","fd","ls","cd","vim","code"
        ];
        let mut best = (usize::MAX, token);
        for k in known { let d = levenshtein(token, k); if d < best.0 { best = (d, k); } }
        if best.0 == 1 { best.1.to_string() } else { token.to_string() }
    }
}
