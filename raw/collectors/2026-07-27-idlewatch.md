# idlewatch — Break Enforcer

> Source: linux/idlewatch
> Collected: 2026-07-27
> Published: Unknown

## Overview

Bash script that monitors X11 idle time and enforces work-break intervals. Turns the screen off during breaks; plays a notification sound when the break ends.

## Requirements

- `xprintidle` — idle time measurement
- `xrandr` — display power control
- `aplay` — notification sound playback
- `flock` (util-linux) — exclusive instance locking

## Configuration

| Variable | Default | Description |
|----------|---------|-------------|
| `POLL_INTERVAL` | 5s | Polling frequency |
| `IDLE_THRESHOLD_S` | 120 | Screen off after 2 min idle |
| `LONG_TIMER_RESET_IDLE_S` | 900 | Reset long break timer if idle 15 min |
| `SHORT_TIMEOUT_S` | 1200 | Short break after 20 min work |
| `LONG_TIMEOUT_S` | 3600 | Long break after 60 min work |
| `SHORT_BREAK_DURATION` | 30s | Short break length |
| `LONG_BREAK_DURATION` | 900s | Long break length |
| `DISPLAY_OUTPUT` | `eDP-1` | Target display |
| `NOTIFICATION_SOUND` | `~/.local/share/sounds/notification.wav` | Break end sound |

## Architecture

- **Main loop**: `sleep(POLL_INTERVAL)` → `xprintidle` → check thresholds → act
- **Lock file**: `$XDG_RUNTIME_DIR/idlewatch.lock` via `flock`
- **Break handling**: `sleep(duration)` in **background** so SIGUSR1 trap can interrupt it
- **Signals**: SIGUSR1 = emergency wake (kills background sleep, restores screen). EXIT trap = always restore screen
- `ponytail: sleep in background + wait` pattern used so signals don't crash the script. The `sleep_pid` is tracked and killed on emergency wake.

## Flow

1. Poll idle time every POLL_INTERVAL seconds
2. If idle > IDLE_THRESHOLD and screen is on → turn screen off
3. If idle >= LONG_TIMER_RESET_IDLE → reset long break timer
4. While screen is off and user is idle → skip accumulating work time
5. While user is active → accumulate short_elapsed + POLL_INTERVAL, long_elapsed + POLL_INTERVAL
6. If long_elapsed >= LONG_TIMEOUT → take_break(LONG_BREAK_DURATION)
7. Elif short_elapsed >= SHORT_TIMEOUT → take_break(SHORT_BREAK_DURATION)

### take_break(duration)
1. Screen off, set `screen_is_off=1`, `abort_break=0`
2. `sleep(duration)` in background, track `sleep_pid`
3. `wait` for sleep to finish (or be killed by SIGUSR1)
4. Screen on, reset timers
5. Play notification sound if break was not aborted
