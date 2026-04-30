# ds2api

Original repository: [CJackHwang/ds2api](https://github.com/CJackHwang/ds2api)

## Quick start

1. Create a DeepSeek account: https://chat.deepseek.com/
2. Install ds2api:
   ```bash
   curl -fsSL https://raw.githubusercontent.com/yookibooki/scripts/main/ds2api/install.sh | bash
   ```
   Config: `~/.local/share/ds2api/config.json`
3. Open http://localhost:5001/admin password `admin`
   - add your accounts
   - generate an API key

base URL: `http://localhost:5001/v1`
see models at `http://localhost:5001/v1/models`

If you need to remove it:
```bash
curl -fsSL https://raw.githubusercontent.com/yookibooki/scripts/main/ds2api/uninstall.sh | bash
```
