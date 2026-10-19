#!/bin/sh
set -u
d=${XDG_RUNTIME_DIR:-/tmp}/transcriber
L=${TRANSCRIBER_MIN_LEVEL:-5}
M=${TRANSCRIBER_MAX_SECS:-300}
mkdir -p "$d"

rp() { cat "$1/pid" 2>/dev/null; }
cm() { cat /proc/$1/comm 2>/dev/null; }
age() { echo $(( $(date +%s) - $(stat -c%Y "$1") )); }
live() {
  p=$(rp "$1")
  [ "$(cm "$p")" = arecord ] && return
  [ -z "$p" ] || kill -0 "$p" 2>/dev/null || return 1
  [ "$(age "$1")" -lt 2 ]
}
k() {
  p=$(rp "$1"); [ -n "$p" ] || return 0
  i=0
  while [ $i -lt 50 ] && [ "$(cm "$p")" = sh ]; do sleep .01; i=$((i+1)); done
  case $(cm "$p") in
    arecord|sh) kill "$p" 2>/dev/null ;;
    *)          return 0 ;;
  esac
  i=0
  while [ $i -lt 100 ] && kill -0 "$p" 2>/dev/null; do sleep .01; i=$((i+1)); done
  kill -9 "$p" 2>/dev/null
}

case ${1:-} in
start)
  for s in "$d"/s.*; do
    [ -d "$s" ] || continue
    live "$s" && [ "$(age "$s")" -le "$M" ] && exit
    k "$s"; rm -rf "$s"
  done
  : ${GROQ_API_KEY:?}
  sd=$d/s.$$; mkdir "$sd" || exit
  b=$$.$RANDOM
  (
    trap 'rm -rf "$sd"' 0
    o=$(
      { printf -- "--$b\r\nContent-Disposition: form-data; name=\"%s\"\r\n\r\n%s\r\n" \
          model whisper-large-v3-turbo language en response_format text temperature 0
        printf -- "--$b\r\nContent-Disposition: form-data; name=\"file\"; filename=\"a.flac\"\r\nContent-Type: audio/flac\r\n\r\n"
        sh -c 'echo $$ >"$1/pid"; exec arecord -q -V mono -f S16_LE -r 16000 -c 1 -t raw 2>"$1/vu"' sh "$sd" |
          flac -sc --force-raw-format --endian=little --sign=signed --channels=1 --bps=16 --sample-rate=16000 - 2>/dev/null
        printf '\r\n--%s--\r\n' "$b"
      } | curl -sSf --http2 --tcp-nodelay --max-time $((M+60)) -X POST -T - -H 'Expect:' \
            -H "Authorization: Bearer $GROQ_API_KEY" \
            -H "Content-Type: multipart/form-data; boundary=$b" \
            https://api.groq.com/openai/v1/audio/transcriptions
    )
    grep -o '[0-9]*%' "$sd/vu" 2>/dev/null |
      awk -v t="$L" '{n++;sub(/%/,"");$1>=t&&l++}END{exit !(n==0||l>=2)}' || exit
    [ -n "$o" ] && printf '%s' "$o" | xclip -sel c && xdotool key --clearmodifiers ctrl+v
  ) </dev/null >/dev/null 2>&1 &
  ;;
stop)
  for s in "$d"/s.*; do
    [ -d "$s" ] && live "$s" || continue
    k "$s"
  done
  ;;
esac
