#!/bin/sh
set -u
d=${XDG_RUNTIME_DIR:-/tmp}/transcriber
s=$d/s
L=${TRANSCRIBER_MIN_LEVEL:-5}
M=${TRANSCRIBER_MAX_SECS:-300}

case ${1:-} in
start)
  : ${GROQ_API_KEY:?}
  mkdir -p "$d" || exit
  exec 9>>"$d/lock"
  flock -n 9 || exit
  rm -rf "$s"; mkdir "$s" && mkfifo "$s/p" || exit
  (
    trap 'rm -rf "$s"' 0
    b=t$$
    arecord -q -V mono -f S16_LE -r 16000 -c 1 -t raw -d "$M" >"$s/p" 2>"$s/vu" &
    r=$!
    echo $r >"$s/rec"
    {
      printf -- "--$b\r\nContent-Disposition: form-data; name=\"%s\"\r\n\r\n%s\r\n" \
        model whisper-large-v3-turbo language en response_format text
      printf -- "--$b\r\nContent-Disposition: form-data; name=\"file\"; filename=\"a.flac\"\r\nContent-Type: audio/flac\r\n\r\n"
      flac -sc --force-raw-format --endian=little --sign=signed --channels=1 --bps=16 --sample-rate=16000 - <"$s/p" 2>/dev/null
      printf '\r\n--%s--\r\n' "$b"
    } | curl -sf --http2 -X POST -T - -H 'Expect:' \
          -H "Authorization: Bearer $GROQ_API_KEY" \
          -H "Content-Type: multipart/form-data; boundary=$b" \
          https://api.groq.com/openai/v1/audio/transcriptions >"$s/o" &
    c=$!
    wait $r
    grep -o '[0-9][0-9]*%' "$s/vu" |
      awk -v t="$L" '{n++;sub(/%/,"");$1>=t&&l++}END{exit !(n==0||l>=2)}' ||
      { kill $c 2>/dev/null; exit; }
    i=0
    while [ $i -lt 140 ] && kill -0 $c 2>/dev/null; do sleep .05; i=$((i+1)); done
    kill -0 $c 2>/dev/null && kill $c
    wait $c || exit
    o=$(sed 's/^[[:space:]]*//;s/[[:space:]]*$//' "$s/o")
    [ -n "$o" ] && printf %s "$o" | xclip -sel c 9>&- && xdotool key --clearmodifiers ctrl+v
  ) </dev/null >/dev/null 2>&1 &
  ;;
stop)
  kill "$(cat "$s/rec" 2>/dev/null)" 2>/dev/null
  ;;
esac
