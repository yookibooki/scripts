#!/bin/sh
set -u
d=${XDG_RUNTIME_DIR:-/tmp}/transcriber
w=$d/a.wav
a=$d/a.flac
mkdir -p "$d"

case ${1:-} in
start)
  pgrep -f "arecord.*$w" >/dev/null && exit
  rm -f "$w" "$a"
  arecord -q -f S16_LE -r 16000 -c 1 -t wav "$w" 2>/dev/null &
  ;;
stop)
  pkill -TERM -f "arecord.*$w"
  i=0
  while [ $i -lt 60 ] && pgrep -f "arecord.*$w" >/dev/null; do sleep 0.05; i=$((i+1)); done
  [ -s "$w" ] || { rm -f "$w"; exit 0; }
  flac -s -f --ignore-chunk-sizes -o "$a" "$w" 2>/dev/null
  [ -s "$a" ] || cp "$w" "$a"
  t=$(curl -sS --http3 --max-time 120 \
        https://api.groq.com/openai/v1/audio/transcriptions \
        -H "Authorization: Bearer $GROQ_API_KEY" \
        -F model=whisper-large-v3-turbo -F file=@"$a" -F language=en \
      | jq -r '.text // empty')
  rm -f "$w" "$a"
  [ -n "$t" ] || exit 0
  printf %s "$t" | xclip -selection clipboard >/dev/null 2>&1
  xdotool key --clearmodifiers ctrl+v
  ;;
esac
