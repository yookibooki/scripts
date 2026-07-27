# idlewatch

> Sources: linux/idlewatch, 2026-07-27
> Raw: [idlewatch](../../raw/collectors/linux-idlewatch.sh)
> Updated: 2026-07-27

A bash script that monitors X11 idle time and turns the screen off after idle periods, then enforces short and long work breaks by blocking the display for a configured duration.

## Overview

`idlewatch` is an X11 idle-monitor daemon written in bash. It polls `xprintidle` every 5 seconds to determine how long the user has been inactive. When idle time exceeds 2 minutes (120 s), it turns the display off via `xrandr`. After 15 minutes (900 s) of continuous idle, it resets the long-break timer so that the long break does not trigger while the user is away. When 20 minutes (1200 s) of active (non-idle) work accumulate, it enforces a 30-second short break; after 60 minutes (3600 s) of active work, it enforces a 15-minute long break. During a break, the screen stays off; sending `SIGUSR1` kills the break early and skips the notification sound.

## Configuration

All configuration is at the top of the script as shell variables.

| Variable | Value | Description |
|----------|-------|-------------|
| `LOCK_FILE` | `${XDG_RUNTIME_DIR:-/tmp}/idlewatch.lock` | Path for exclusive execution lock |
| `DISPLAY_OUTPUT` | `eDP-1` | X randr output name to control |
| `NOTIFICATION_SOUND` | `$HOME/.local/share/sounds/notification.wav` | Sound played at break end |
| `POLL_INTERVAL` | `5` | Seconds between idle checks |
| `IDLE_THRESHOLD_S` | `120` | Idle seconds before screen turns off |
| `LONG_TIMER_RESET_IDLE_S` | `900` | Idle seconds that reset the long break timer |
| `SHORT_TIMEOUT_S` | `1200` | Active work seconds before a short break |
| `LONG_TIMEOUT_S` | `3600` | Active work seconds before a long break |
| `SHORT_BREAK_DURATION` | `30` | Seconds to hold the screen off for a short break |
| `LONG_BREAK_DURATION` | `900` | Seconds to hold the screen off for a long break |

## Dependencies

The script requires three commands to be present on `PATH` at startup; it exits with an error if any are missing:

- `xprintidle` — reports X11 idle time in milliseconds
- `xrandr` — turns the display output on or off
- `aplay` — plays the notification sound file

## Single-Instance Locking

The script opens file descriptor 9 on `$LOCK_FILE` and acquires an exclusive `flock`. A second invocation exits immediately with the message "Script is already running."

## Break Logic

`take_break` is the core routine for enforcing breaks:

1. Turns the screen off (`xrandr --output eDP-1 --off`).
2. Sets `screen_is_off=1` and resets `abort_break=0`.
3. Runs `sleep DURATION` in the background (so the `EXIT` trap can still fire).
4. Waits for the sleep to finish.
5. Restores the screen (`xrandr --output eDP-1 --auto`).
6. Resets `short_elapsed` to 0.
7. Plays the notification sound **only if** `abort_break` is 0 (i.e., no `SIGUSR1` was received during the break).

## Traps

| Signal | Action |
|--------|--------|
| `SIGUSR1` | Calls `emergency_wake` — sets `abort_break=1`, kills the in-progress `sleep`, so the break ends immediately |
| `EXIT` | Calls `set_screen "auto"` — guarantees the display is restored even if the script is killed or crashes |

## Main Loop

Each iteration of the `while true` loop:

1. **Safe sleep** — `sleep POLL_INTERVAL` runs in the background; the script `wait`s for it. Backgrounding means `SIGUSR1` is never lost while the script is sleeping.
2. **Read idle time** — calls `xprintidle`; if the value is not a non-negative integer (e.g., X11 is unavailable), the tick is skipped.
3. **Screen off → user active** — if `screen_is_off` and idle < `IDLE_THRESHOLD_S`, restore the screen.
4. **User is idle** — if idle >= `IDLE_THRESHOLD_S`, turn the screen off; reset `short_elapsed`; if idle >= `LONG_TIMER_RESET_IDLE_S`, also reset `long_elapsed`; skip work-time accumulation.
5. **Accumulate active work** — increment `short_elapsed` and `long_elapsed` by `POLL_INTERVAL`.
6. **Enforce breaks** — if `long_elapsed` >= `LONG_TIMEOUT_S`, take a `LONG_BREAK_DURATION`-second long break (reset both timers). Else if `short_elapsed` >= `SHORT_TIMEOUT_S`, take a `SHORT_BREAK_DURATION`-second short break (reset short timer only).