#!/usr/bin/env bash
set -euo pipefail

PACMAN_PKGS=(aichat alsa-utils base base-devel brightnessctl btop chezmoi dmenu docker docker-compose efibootmgr fontconfig git gitui i3-wm intel-media-driver intel-ucode iwd jq libva-intel-driver libxft linux linux-firmware mesa neovim noto-fonts-emoji openssh otf-firamono-nerd pass postgresql prettier redshift reflector ruff shfmt sudo tmux unzip uv vulkan-intel xclip xorg-server xorg-xinit xorg-xsetroot xorg-xwininfo)
AUR_PKGS=(antigravity cli-proxy-api-bin windsurf brave-bin)

[[ $EUID -ne 0 ]] || exit 1
sudo -v

sudo pacman -Syu --noconfirm --needed "${PACMAN_PKGS[@]}"

git clone --depth 1 https://aur.archlinux.org/yay-bin.git && (cd yay-bin && makepkg -si --noconfirm) && rm -rf yay-bin

yay -S --noconfirm --needed "${AUR_PKGS[@]}"

git clone --depth 1 https://git.suckless.org/st "$HOME/.local/src/st"
sudo make -C "$HOME/.local/src/st" clean install

sudo usermod -aG docker "$USER"

sudo systemctl enable --now alsa-state.service || true

sudo mkdir -p /etc/systemd/system/getty@tty1.service.d
sudo tee /etc/systemd/system/getty@tty1.service.d/autologin.conf >/dev/null <<EOF
[Service]
ExecStart=
ExecStart=-/usr/bin/agetty --autologin $USER --noreset --noclear - %I \$TERM
EOF

curl -fsSL https://fnm.vercel.app/install | bash -s -- --skip-shell
. "$HOME/.bashrc"
fnm i --lts

sudo systemctl daemon-reload

echo "Done. Reboot."
