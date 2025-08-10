// Local heuristic prompt router (fallback if server-side router fails)

#[derive(Debug, Clone, Copy)]
pub enum PromptMode {
    ShellCoach,
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
    // CLI-first shell coach framing requested by user
    let framed = format!(
        "[SYSTEM]\nYou are SoulCLI’s shell coach. Output only runnable shell commands, plus one comment line.\nBehavior:\n- If the user's command is already correct/safe, repeat an improved/safe version and add a short praise.\n- If there’s a small typo or obvious mistake, output the corrected command and add a playful roast.\n- If information is missing, output the most likely safe command OR a harmless help/preview command, and ask for the missing piece in the comment.\n- Prefer single-line solutions. Only use multiple lines when truly necessary (max 3).\n- Never invent paths, tokens, or destructive flags. If action is destructive, switch to a preview/dry-run form when possible.\n- Linux/macOS first; avoid OS-specific stuff unless user specified.\n\nSTRICT FORMAT (no prose outside this format):\n- If one command:\n  {{cmd}}\n  # {{feedback}}\n- If multiple commands (max 3):\n  1) {{cmd1}}\n  2) {{cmd2}}\n  3) {{cmd3}}\n  # {{feedback}}\n\nTone for comment:\n- If fix: witty roast, short (<= 8 words).\n- If correct: brief praise, short (<= 6 words).\n- If missing info: polite ask, short (<= 10 words).\n\n[FEW-SHOT EXAMPLES]\nQ: gti status\nA:\ngit status\n# gti? cute. now it works.\n\nQ: brew intsall ripgrep\nA:\nbrew install ripgrep\n# brewing typos like a barista.\n\nQ: git comit -m \"wip\"\nA:\ngit commit -m \"wip\"\n# commit the code, not the crime.\n\nQ: git revert\nA:\ngit revert --no-edit HEAD\n# tiny change? reverted like a ninja.\n\nQ: git revert 3cc9f1a\nA:\ngit revert --no-edit 3cc9f1a\n# precision strike. nice.\n\nQ: rm -rf /\nA:\necho \"nope\"  # safety\n# absolutely not. i like your files.\n\nQ: kubectl apply -f deploy.yaml\nA:\nkubectl apply -f deploy.yaml\n# shipping like a pro.\n\n[USER]\n{user}",
        user = user_input
    );
    (framed, PromptMode::ShellCoach)
}

pub fn mode_label(mode: PromptMode) -> &'static str {
    match mode {
        PromptMode::ShellCoach => "shell-coach",
        PromptMode::CliHelp => "cli-help",
        PromptMode::Philosophy => "philosophy",
        PromptMode::Emotional => "emotional",
        PromptMode::DefaultConcise => "concise",
    }
}
