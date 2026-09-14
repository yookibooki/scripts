#!/bin/sh
set -u
d=${XDG_RUNTIME_DIR:-/tmp}/transcriber
mkdir -p "$d"
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
  t=$(curl -sS https://api.groq.com/openai/v1/audio/transcriptions \
      -H "Authorization: Bearer $GROQ_API_KEY" \
      -F model="whisper-large-v3-turbo" -F file=@"$d/a.wav" | jq -r .text)
  rm -f "$d/a.wav"
  case $t in ''|null) ;; *)
    printf %s "$t" | xclip -selection clipboard -i
    xdotool key --clearmodifiers ctrl+v ;;
  esac
  ;;
esac
