# hop demo — recorded with VHS (https://github.com/charmbracelet/vhs)
# Regenerate with: ./demo/record.sh   (seeds a sandbox, builds, then records)
#
# The recording runs against a throwaway $HOME seeded by demo/seed.py, so it
# never touches real sessions and is fully reproducible.

Output demo/hop.gif

Set Shell "bash"
Set FontFamily "JetBrainsMono Nerd Font"
Set FontSize 20
Set Width 1600
Set Height 1000
Set Padding 24
Set TypingSpeed 85ms

# Pure-black background with a Catppuccin-Mocha palette for the TUI's colors.
Set Theme { "background": "#000000", "foreground": "#cdd6f4", "cursor": "#f5e0dc", "selection": "#313244", "black": "#45475a", "red": "#f38ba8", "green": "#a6e3a1", "yellow": "#f9e2af", "blue": "#89b4fa", "magenta": "#f5c2e7", "cyan": "#94e2d5", "white": "#bac2de", "brightBlack": "#585b70", "brightRed": "#f38ba8", "brightGreen": "#a6e3a1", "brightYellow": "#f9e2af", "brightBlue": "#89b4fa", "brightMagenta": "#f5c2e7", "brightCyan": "#94e2d5", "brightWhite": "#a6adc8" }

# --- Sandbox setup (hidden): local binary, isolated $HOME, sit inside the repo,
#     and a pretty git-aware prompt -----------------------------------------
Hide
Type "export PATH=$PWD/target/release:$PATH" Enter
Type "export HOME=$PWD/demo/.demo-home" Enter
Type `cd "$HOME/work/hop"` Enter
Type `export PS1='\[\e[38;5;212m\]hop\[\e[0m\] \[\e[38;5;245m\]on \[\e[38;5;111m\]main\[\e[0m\] \[\e[38;5;156m\]❯\[\e[0m\] '` Enter
Type "clear" Enter
Show

# --- Launch bare `hop`: auto-scopes to the current repo (repo:amittamari/hop),
#     index streams in, the repo's Claude + Codex sessions render -------------
Sleep 700ms
Type "hop"
Sleep 800ms
Enter
Sleep 2.8s

# --- Live full-text search, narrowing within the auto-scoped repo ------------
Type "preview"
Sleep 2.6s

# --- Browse results without preview -----------------------------------------
Down
Sleep 1.2s

# --- Toggle the preview pane on; it renders for the selected session --------
Ctrl+P
Sleep 1.8s

# --- Preview: jump to the next highlighted match, then scroll ----------------
Ctrl+N
Sleep 1.2s
Ctrl+D
Sleep 1.4s

# --- Resume the selected session (confirmation modal) ------------------------
Up
Sleep 900ms
Enter
Sleep 2.6s
