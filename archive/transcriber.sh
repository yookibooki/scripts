#!/bin/sh
set -u
d=${XDG_RUNTIME_DIR:-/tmp}/transcriber
mkdir -p "$d"
api=https://api.groq.com/openai/v1

case ${1:-} in
start)
  [ -e "$d/pid" ] && exit
  arecord -q -f S16_LE -r 16000 -c 1 -t wav "$d/a.wav" &
  echo $! > "$d/pid"
  ;;
stop)
  [ -f "$d/pid" ] || exit
  p=$(cat "$d/pid"); rm -f "$d/pid"
  kill -INT "$p" 2>/dev/null; wait "$p" 2>/dev/null

  t=$(curl -sS --http2 "$api/audio/transcriptions" \
        -H "Authorization: Bearer $GROQ_API_KEY" \
        -F model="whisper-large-v3-turbo" \
        -F language="en" \
        -F file=@"$d/a.wav" \
      | jq -r '.text // empty')
  rm -f "$d/a.wav"

  t=$(printf '%s' "$t" | sed 's/^[[:space:]]*//')
  [ -n "$t" ] || exit

  printf %s "$t" | xclip -selection clipboard -i
  xdotool key --clearmodifiers ctrl+v
  ;;
esac
