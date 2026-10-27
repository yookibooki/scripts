#!/usr/bin/env bash
# xrandr --off (not `xset dpms force off`): --off stays off until explicit
# --auto, forcing eyes off screen. dpms wakes on any input.
LOCK_FILE="${XDG_RUNTIME_DIR:-/tmp}/idlewatch.lock"
DISPLAY_OUTPUT="${IDLEWATCH_OUTPUT:-eDP-1}"
POLL_INTERVAL=5
IDLE_THRESHOLD_S=120
LONG_TIMER_RESET_IDLE_S=900
SHORT_TIMEOUT_S=1200
LONG_TIMEOUT_S=3600
SHORT_BREAK_DURATION=30
LONG_BREAK_DURATION=900

for cmd in xprintidle xrandr; do
    command -v "$cmd" >/dev/null || {
        echo "Error: Required command '$cmd' is not installed." >&2
        exit 1
    }
done

exec 9>"$LOCK_FILE" || exit 1
flock -n 9 || {
    echo "Script is already running."
    exit 0
}

screen_is_off=0
sleep_pid=""
short_start=$SECONDS
long_start=$SECONDS

set_screen() {
    local state="$1"
    xrandr --output "$DISPLAY_OUTPUT" --"$state" &>/dev/null || true
}

screen_off() {
    (( screen_is_off == 0 )) || return 0
    set_screen "off"
    screen_is_off=1
}

screen_on() {
    (( screen_is_off == 1 )) || return 0
    set_screen "auto"
    screen_is_off=0
}

interruptible_sleep() {
    local duration="$1"
    sleep "$duration" 9>&- &
    sleep_pid=$!
    wait "$sleep_pid" 2>/dev/null
    local status=$?
    sleep_pid=""
    return "$status"
}

emergency_wake() {
    if [[ -n "$sleep_pid" ]]; then
        kill "$sleep_pid" 2>/dev/null || true
    fi
}

take_break() {
    local duration="$1"
    screen_off
    interruptible_sleep "$duration" || true
    screen_on
    short_start=$SECONDS
}

trap 'emergency_wake' SIGUSR1
trap 'set_screen "auto"' EXIT

while true; do
    interruptible_sleep "$POLL_INTERVAL"

    idle_ms=$(xprintidle 2>/dev/null)
    [[ "$idle_ms" =~ ^[0-9]+$ ]] || continue
    idle_s=$(( idle_ms / 1000 ))

    if (( idle_s >= IDLE_THRESHOLD_S )); then
        screen_off
        short_start=$SECONDS
        if (( idle_s >= LONG_TIMER_RESET_IDLE_S )); then
            long_start=$SECONDS
        fi
        continue
    fi

    screen_on

    if (( SECONDS - long_start >= LONG_TIMEOUT_S )); then
        long_start=$SECONDS
        take_break "$LONG_BREAK_DURATION"
    elif (( SECONDS - short_start >= SHORT_TIMEOUT_S )); then
        take_break "$SHORT_BREAK_DURATION"
    fi
done
