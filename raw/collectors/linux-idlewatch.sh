#!/usr/bin/env bash

# ==========================================
# Configuration
# ==========================================
LOCK_FILE="${XDG_RUNTIME_DIR:-/tmp}/idlewatch.lock"
DISPLAY_OUTPUT="eDP-1"
NOTIFICATION_SOUND="$HOME/.local/share/sounds/notification.wav"

POLL_INTERVAL=5

# Thresholds (in seconds)
IDLE_THRESHOLD_S=120           # 2 minutes: Turn screen off if idle
LONG_TIMER_RESET_IDLE_S=900    # 15 minutes: Reset long break timer if idle this long

# Active Work Time (in seconds)
SHORT_TIMEOUT_S=1200           # 20 minutes of work triggers a short break
LONG_TIMEOUT_S=3600            # 60 minutes of work triggers a long break

# Break Durations (in seconds)
SHORT_BREAK_DURATION=30        # 30 seconds
LONG_BREAK_DURATION=900        # 15 minutes

# ==========================================
# Initialization & Checks
# ==========================================
for cmd in xprintidle xrandr aplay; do
    if ! command -v "$cmd" &>/dev/null; then
        echo "Error: Required command '$cmd' is not installed." >&2
        exit 1
    fi
done

# Exclusive execution locking
exec 9>"$LOCK_FILE" || exit 1
flock -n 9 || { echo "Script is already running."; exit 0; }

# ==========================================
# State Variables
# ==========================================
short_elapsed=0
long_elapsed=0
screen_is_off=0
sleep_pid=""
abort_break=0

# ==========================================
# Helper Functions
# ==========================================
set_screen() {
    local state="$1" # "auto" or "off"
    xrandr --output "$DISPLAY_OUTPUT" --"$state" &>/dev/null || true
}

play_notification() {
    if [[ -f "$NOTIFICATION_SOUND" ]]; then
        aplay -q "$NOTIFICATION_SOUND" &>/dev/null &
    fi
}

emergency_wake() {
    abort_break=1
    if [[ -n "$sleep_pid" ]]; then
        kill "$sleep_pid" 2>/dev/null
    fi
}

take_break() {
    local duration="$1"

    set_screen "off"
    screen_is_off=1
    abort_break=0

    # Sleep in the background so the trap can safely interrupt it
    sleep "$duration" &
    sleep_pid=$!
    wait "$sleep_pid" 2>/dev/null
    sleep_pid=""

    # Restore screen
    set_screen "auto"
    screen_is_off=0
    short_elapsed=0

    if (( abort_break == 0 )); then
        play_notification
    fi
}

# ==========================================
# Traps
# ==========================================
# Bind emergency wake to SIGUSR1
trap 'emergency_wake' SIGUSR1

# Ensure screen turns back on if the script is killed, exits, or crashes
trap 'set_screen "auto"' EXIT

# ==========================================
# Main Loop
# ==========================================
while true; do
    # 1. Safe Polling: We sleep at the top of the loop in the background.
    # This guarantees that receiving a signal won't crash the script.
    sleep "$POLL_INTERVAL" &
    sleep_pid=$!
    wait "$sleep_pid" 2>/dev/null
    sleep_pid=""

    # 2. Safely read idle time. If X11 is inaccessible, skip this tick.
    idle_ms=$(xprintidle 2>/dev/null)
    if ! [[ "$idle_ms" =~ ^[0-9]+$ ]]; then
        continue
    fi

    idle_s=$(( idle_ms / 1000 ))

    # 3. Screen is off, but user became active
    if (( screen_is_off == 1 )) && (( idle_s < IDLE_THRESHOLD_S )); then
        set_screen "auto"
        screen_is_off=0
    fi

    # 4. User is currently idle
    if (( idle_s >= IDLE_THRESHOLD_S )); then
        if (( screen_is_off == 0 )); then
            set_screen "off"
            screen_is_off=1
        fi

        short_elapsed=0

        if (( idle_s >= LONG_TIMER_RESET_IDLE_S )); then
            long_elapsed=0
        fi
        continue # Skip accumulating work time while idle
    fi

    # 5. Accumulate active work time
    (( short_elapsed += POLL_INTERVAL ))
    (( long_elapsed  += POLL_INTERVAL ))

    # 6. Enforce breaks
    if (( long_elapsed >= LONG_TIMEOUT_S )); then
        long_elapsed=0
        take_break "$LONG_BREAK_DURATION"
    elif (( short_elapsed >= SHORT_TIMEOUT_S )); then
        take_break "$SHORT_BREAK_DURATION"
    fi
done
