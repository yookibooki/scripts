### Install setup
```bash
curl -fsSL https://arch.yooki.workers.dev | bash
```

### `archinstall` disk error fix
```bash
umount -R /mnt 2>/dev/null
swapoff -a 2>/dev/null
wipefs -af /dev/sda
sgdisk --zap-all /dev/sda
partprobe /dev/sda
udevadm settle
```

### Brave flags
- **Brave News Feed Update**  
- **Brave News prompts on New Tab Page**  
- **Brave Rewards Gemini**  
- **Enable Brave Wallet**
- **Enable Gemini for Brave Rewards**
- **Enable Playlist**
- **Enable Zcash support for Brave Wallet**  
- **Enable Bitcoin support for Brave Wallet**  
- **NTP Calendar Module**  
- **NTP Microsoft Authentication Module**  
- **NTP Most Relevant Tab Resumption Module**  
- **NTP Outlook Calendar Module**  
- **NTP Sharepoint Module**  
- **Override download danger level**  
- **Parallel downloading**

**Search engines**
```bash
https://www.google.com/search?q=%s&udm=50
https://chatgpt.com/?prompt=%s
https://github.com/search?q=%s&s=stars&o=desc
https://yandex.ru/search/?text=%s
https://www.youtube.com/results?search_query=%s
```