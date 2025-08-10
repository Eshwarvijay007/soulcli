// Local heuristic prompt router (fallback if server-side router fails)

#[derive(Debug, Clone, Copy)]
pub enum PromptMode {
    CliHelp,
    Philosophy,
    Emotional,
    DefaultConcise,
}

fn contains_any(haystack: &str, needles: &[&str]) -> bool {
    needles.iter().any(|n| haystack.contains(n))
}

fn is_cli_help_query(text: &str) -> bool {
    let t = text.to_lowercase();
    contains_any(&t, &[
        "how to", "command", "flag", "usage", "error", "permission", "not found",
        "bash", "zsh", "terminal", "shell", "linux", "mac", "windows",
        "git ", "docker", "kubectl", "npm", "pnpm", "yarn", "pip", "brew ",
        "ls ", "cd ", "rm ", "mv ", "cp ", "grep ", "awk ", "sed ", "ps ", "kill ", "systemctl",
        "--help", "help me", "fix", "install", "build", "run", "compile",
    ]) || text.contains('`')
}

fn is_philosophy_query(text: &str) -> bool {
    let t = text.to_lowercase();
    contains_any(&t, &[
        "philosophy", "philosophical", "meaning of life", "ethics", "virtue",
        "metaphysics", "epistemology", "stoic", "plato", "aristotle", "kant",
        "nietzsche", "heidegger", "existential", "what is truth", "why do we",
    ])
}

fn is_emotional_or_story(text: &str) -> bool {
    let t = text.to_lowercase();
    contains_any(&t, &[
        "story", "poem", "lyric", "song", "love", "sad", "happy", "angry", "comfort",
        "encourage", "emotional", "tell me a story", "write a story",
    ])
}

pub fn route_prompt(user_input: &str) -> (String, PromptMode) {
    let mode = if is_cli_help_query(user_input) {
        PromptMode::CliHelp
    } else if is_philosophy_query(user_input) {
        PromptMode::Philosophy
    } else if is_emotional_or_story(user_input) {
        PromptMode::Emotional
    } else {
        PromptMode::DefaultConcise
    };

    let framed = match mode {
        PromptMode::CliHelp => format!(
            "[SYSTEM]\nYou are a concise command-line assistant.\nRules:\n- Return exact command(s) only, minimal prose.\n- Prefer one-liners.\n- If multiple steps are required, list numbered short steps.\n- Keep the answer under 2 lines unless strictly necessary.\nIf you detect common typos (like 'gti' instead of 'git'), correct them and include a gentle, funny quip.\n\n[USER]\n{}",
            user_input
        ),
        PromptMode::Philosophy => format!(
            "[SYSTEM]\nYou are a philosophical guide.\nRules:\n- Be thoughtful yet succinct (<= 5 lines).\n- Optionally mention relevant schools or thinkers.\n- Avoid fluff.\n\n[USER]\n{}",
            user_input
        ),
        PromptMode::Emotional => format!(
            "[SYSTEM]\nYou are an empathetic storyteller.\nRules:\n- Write a short, vivid response (6–10 lines).\n- Keep an emotional, human tone.\n\n[USER]\n{}",
            user_input
        ),
        PromptMode::DefaultConcise => format!(
            "[SYSTEM]\nAnswer succinctly in <= 2 lines.\nIf the user made a CLI typo, correct it and include a funny one-liner.\n\n[USER]\n{}",
            user_input
        ),
    };

    (framed, mode)
}

pub fn mode_label(mode: PromptMode) -> &'static str {
    match mode {
        PromptMode::CliHelp => "cli-help",
        PromptMode::Philosophy => "philosophy",
        PromptMode::Emotional => "emotional",
        PromptMode::DefaultConcise => "concise",
    }
}
