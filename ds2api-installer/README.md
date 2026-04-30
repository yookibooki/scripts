# ds2api installer

One-line install/update (Linux/macOS):

```bash
curl -fsSL https://raw.githubusercontent.com/yookibooki/scripts/main/ds2api-installer/ds2api-installer.sh | bash
```

Uninstall:

```bash
curl -fsSL https://raw.githubusercontent.com/yookibooki/scripts/main/ds2api-installer/ds2api-installer.sh | bash -s -- uninstall
```

Defaults:
- install root: `~/.local/share/ds2api`
- binary symlink: `~/.local/bin/ds2api`

The installer always fetches the latest DS2API release, verifies `sha256sums.txt`, preserves `config.json`, and updates the `current` symlink.

CI:
- `ds2api-installer.sh` is syntax-checked on push / PR
