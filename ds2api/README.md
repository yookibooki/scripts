# ds2api installer

One-line install/update (Linux/macOS):

```bash
curl -fsSL https://raw.githubusercontent.com/yookibooki/scripts/main/ds2api/install.sh | bash
```

Uninstall:

```bash
curl -fsSL https://raw.githubusercontent.com/yookibooki/scripts/main/ds2api/install.sh | bash -s -- uninstall
```

Defaults:
- install root: `~/.local/share/ds2api`
- binary symlink: `~/.local/bin/ds2api`

The launcher automatically points DS2API at `~/.local/share/ds2api/config.json`.
The installer always fetches the latest DS2API release, verifies `sha256sums.txt`, preserves `config.json`, and updates the `current` symlink.

CI:
- `install.sh` is syntax-checked on push / PR
