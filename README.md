# scripts

Reusable shell scripts.

## ds2api installer

One-line install/update (Linux/macOS):

```bash
curl -fsSL https://raw.githubusercontent.com/yookibooki/scripts/main/ds2api-installer.sh | bash
```

Update to a specific version:

```bash
curl -fsSL https://raw.githubusercontent.com/yookibooki/scripts/main/ds2api-installer.sh | bash -s -- --tag v4.1.2
```

Uninstall:

```bash
curl -fsSL https://raw.githubusercontent.com/yookibooki/scripts/main/ds2api-installer.sh | bash -s -- uninstall
```

Defaults:
- install root: `~/.local/share/ds2api`
- binary symlink: `~/.local/bin/ds2api`

Env overrides:
- `DS2API_REPO`
- `DS2API_INSTALL_ROOT`
- `DS2API_BIN_DIR`

The installer downloads the matching GitHub release asset, verifies `sha256sums.txt`, preserves `config.json`, and updates the `current` symlink.

CI:
- `ds2api-installer.sh` is syntax-checked on push / PR
