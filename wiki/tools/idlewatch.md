# idlewatch — X11 Idle Monitor and Break Timer

> Sources: linux/idlewatch, 2026-07-27
> Raw: [linux/idlewatch](../raw/collectors/linux-idlewatch.sh)
> Updated: 2026-07-27

## Overview

A bash script that monitors X11 idle time via `xprintidle` and enforces break scheduling. The script turns the screen off after a period of inactivity and forces short/long work breaks to promote healthy usage patterns.

## Configuration

| Parameter | Default | Description |
|-----------|---------|-------------|
| Idle threshold | 120 seconds | Time of inactivity before screen off |
| Short break | 30 seconds | Break after 20 minutes active |
| Long break | 300 seconds | Break after extended work period |

## Behavior

1. **Screen off**: After 120 seconds of inactivity (no input events), the X11 screen is locked/suspended via `xset dpms force off`.
2. **Short break**: After 20 minutes of continuous active use, a 30-second break reminder is displayed.
3. **Resume**: Any keyboard or mouse input resumes normal operation.

## Dependencies

- `xprintidle` — reads X11 idle time in milliseconds
- `xset` — DPMS/screen control
- Standard Unix utilities (`awk`, `grep`, `sleep`)

## Installation

```bash
cp idlewatch.sh ~/.local/bin/idlewatch
chmod +x ~/.local/bin/idlewatch
```

## Changelog

| Date | Change |
|------|--------|
| 2026-07-27 | Initial idlewatch documentation |
