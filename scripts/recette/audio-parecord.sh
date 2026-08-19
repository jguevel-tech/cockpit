#!/bin/bash
# Quelle commande parecord capte le micro et le son systeme, sans connaitre le nom des
# peripheriques ? Teste les formes generiques, dans une configuration PulseAudio reelle.
set -u
export XDG_RUNTIME_DIR=/tmp/run-recette
mkdir -p "$XDG_RUNTIME_DIR"; chmod 700 "$XDG_RUNTIME_DIR"
pulseaudio --start --exit-idle-time=-1 --log-target=file:/tmp/pulse.log >/dev/null 2>&1
sleep 2
pactl load-module module-null-sink sink_name=faux >/dev/null
pactl load-module module-virtual-source source_name=micro master=faux.monitor >/dev/null 2>&1
pactl set-default-sink faux >/dev/null 2>&1
pactl set-default-source micro >/dev/null 2>&1

essai() {
  local nom="$1"; shift
  timeout 3 "$@" > /tmp/out.raw 2>/tmp/out.err
  printf "%-42s octets=%-8s %s\n" "$nom" "$(stat -c %s /tmp/out.raw)" "$(head -c 90 /tmp/out.err | tr '\n' ' ')"
}

echo "### MICRO ###"
essai "parecord (source par defaut)" parecord --rate=16000 --channels=1 --format=s16le --raw
essai "parecord --device=@DEFAULT_SOURCE@" parecord --device=@DEFAULT_SOURCE@ --rate=16000 --channels=1 --format=s16le --raw
echo
echo "### SON SYSTEME ###"
essai "parecord --device=@DEFAULT_MONITOR@" parecord --device=@DEFAULT_MONITOR@ --rate=16000 --channels=1 --format=s16le --raw
essai "parecord --monitor-stream (si connu)" parecord --monitor-stream=0 --rate=16000 --channels=1 --format=s16le --raw
