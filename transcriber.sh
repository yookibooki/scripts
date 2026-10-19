#!/bin/sh
set -u
d=${XDG_RUNTIME_DIR:-/tmp}/transcriber
L=${TRANSCRIBER_MIN_LEVEL:-5}
M=${TRANSCRIBER_MAX_SECS:-300}
mkdir -p "$d"

live() {
  p=$(cat "$1/pid" 2>/dev/null) || return 1
  [ "$(cat /proc/$p/comm 2>/dev/null)" = arecord ] || return 1
  [ "$(awk '{print $3}' /proc/$p/stat 2>/dev/null)" != Z ]
}
k() {
  p=$(cat "$1/pid" 2>/dev/null) || return 0
  kill "$p" 2>/dev/null
  i=0
  while [ $i -lt 100 ] && kill -0 "$p" 2>/dev/null; do sleep .01; i=$((i+1)); done
}

case ${1:-} in
start)
  for s in "$d"/*; do
    [ -d "$s" ] || continue
    live "$s" && exit
    rm -rf "$s"
  done
  : ${GROQ_API_KEY:?}
  sd=$d/$$; mkdir "$sd" || exit
  b=$$.$RANDOM
  (
    trap 'rm -rf "$sd"' 0
    o=$(
      { printf -- "--$b\r\nContent-Disposition: form-data; name=\"%s\"\r\n\r\n%s\r\n" \
          model whisper-large-v3-turbo language en response_format text
        printf -- "--$b\r\nContent-Disposition: form-data; name=\"file\"; filename=\"a.flac\"\r\nContent-Type: audio/flac\r\n\r\n"
        sh -c 'echo $$ >"$1/pid"; exec arecord -q -V mono -f S16_LE -r 16000 -c 1 -t raw 2>"$1/vu"' sh "$sd" |
          flac -sc --force-raw-format --endian=little --sign=signed --channels=1 --bps=16 --sample-rate=16000 - 2>/dev/null
        printf '\r\n--%s--\r\n' "$b"
      } | curl -sSf --http2 --max-time $((M+60)) -X POST -T - -H 'Expect:' \
            -H "Authorization: Bearer $GROQ_API_KEY" \
            -H "Content-Type: multipart/form-data; boundary=$b" \
            https://api.groq.com/openai/v1/audio/transcriptions
    )
    grep -o '[0-9]*%' "$sd/vu" 2>/dev/null |
      awk -v t="$L" '{n++;sub(/%/,"");$1>=t&&l++}END{exit !(n==0||l>=2)}' || exit
    [ -n "$o" ] && printf %s "$o" | xclip -sel c && xdotool key --clearmodifiers ctrl+v
  ) </dev/null >/dev/null 2>&1 &
  ;;
stop)
  for s in "$d"/*; do
    if [ -d "$s" ] && live "$s"; then k "$s"; fi
  done
  ;;
esac
