## Quick start

1. Create a DeepSeek account: https://chat.deepseek.com/
2. Install [ds2api](https://github.com/CJackHwang/ds2api):
   ```bash
   curl -fsSL https://raw.githubusercontent.com/yookibooki/scripts/main/ds2api/install.sh | bash
   ```
   Config: `~/.local/share/ds2api/config.json`
3. Open http://localhost:5001/admin password `admin`
   - add your accounts
   - generate an API key

base URL: `http://localhost:5001/v1`  
see models at `http://localhost:5001/v1/models`

`~/.config/opencode/opencode.json`:
```json
{
  "$schema": "https://opencode.ai/config.json",
  "provider": {
    "ds2api": {
      "npm": "@ai-sdk/openai",
      "name": "DS2API",
      "options": {
        "baseURL": "http://localhost:5001/v1",
        "apiKey": "{env:DS2API_API_KEY}"
      },
      "models": {
        "deepseek-v4-flash": { "name": "DeepSeek V4 Flash" },
        "deepseek-v4-flash-nothinking": {
          "name": "DeepSeek V4 Flash (No Thinking)"
        },
        "deepseek-v4-pro": { "name": "DeepSeek V4 Pro" },
        "deepseek-v4-pro-nothinking": {
          "name": "DeepSeek V4 Pro (No Thinking)"
        },
        "deepseek-v4-flash-search": { "name": "DeepSeek V4 Flash Search" },
        "deepseek-v4-flash-search-nothinking": {
          "name": "DeepSeek V4 Flash Search (No Thinking)"
        },
        "deepseek-v4-pro-search": { "name": "DeepSeek V4 Pro Search" },
        "deepseek-v4-pro-search-nothinking": {
          "name": "DeepSeek V4 Pro Search (No Thinking)"
        },
        "deepseek-v4-vision": { "name": "DeepSeek V4 Vision" },
        "deepseek-v4-vision-nothinking": {
          "name": "DeepSeek V4 Vision (No Thinking)"
        },
        "deepseek-v4-vision-search": { "name": "DeepSeek V4 Vision Search" },
        "deepseek-v4-vision-search-nothinking": {
          "name": "DeepSeek V4 Vision Search (No Thinking)"
        }
      }
    }
  }
}
```

uninstall:
```bash
curl -fsSL https://raw.githubusercontent.com/yookibooki/scripts/main/ds2api/uninstall.sh | bash
```
