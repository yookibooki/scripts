# transcriber.sh — push-to-dictate
Hold Right Ctrl = dictate raw. Hold Right Shift = dictate then condense.
related: ~/.config/i3/config, ~/.local/bin/transcriber.sh (symlink).

# idlewatch.sh — idle monitor + break enforcer
Polls xprintidle every 5s; screen off when idle ≥120s; 30s break per 20m active, 15m break per 60m active; SIGUSR1 aborts break. Env: IDLEWATCH_OUTPUT (default eDP-1), IDLEWATCH_AUDIO.
related: ~/.config/i3/config ($mod+Escape aborts break), ~/.local/bin/idlewatch (symlink), sounds/notification.wav -> ~/.local/share/sounds/notification.wav
