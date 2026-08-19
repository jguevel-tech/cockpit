#!/bin/bash
# Recette audio sur Ubuntu 22.04 : quel outil sait enregistrer quand le serveur est
# PulseAudio ? Repond par l'experience, dans la configuration reelle d'un testeur.
set -u
export XDG_RUNTIME_DIR=/tmp/run-recette
mkdir -p "$XDG_RUNTIME_DIR"; chmod 700 "$XDG_RUNTIME_DIR"

echo "=== demarrage de PulseAudio (serveur audio de 22.04) ==="
pulseaudio --start --exit-idle-time=-1 --log-target=file:/tmp/pulse.log 2>&1 | tail -2
sleep 2
pactl load-module module-null-sink sink_name=faux sink_properties=device.description=Faux > /dev/null
pactl load-module module-virtual-source source_name=micro master=faux.monitor > /dev/null 2>&1 \
  || echo "(module-virtual-source indisponible, le monitor servira de source)"
echo
echo "=== quel serveur audio tourne ? ==="
pactl info 2>&1 | grep -E "Server Name|Server Version"
echo
echo "=== sources vues par PulseAudio ==="
pactl list short sources 2>&1 | head -5
echo
echo "=== ce que PipeWire voit (pw-record est un client PipeWire) ==="
pw-cli info 0 2>&1 | head -3
echo
echo "### TEST 1 : pw-record (ce qu'utilise Cockpit aujourd'hui) ###"
timeout 4 pw-record --rate 16000 --channels 1 --format s16 - > /tmp/pw.raw 2>/tmp/pw.err
echo "code=$? octets=$(stat -c %s /tmp/pw.raw 2>/dev/null)"
echo "erreur: $(head -c 300 /tmp/pw.err)"
echo
echo "### TEST 2 : parecord (client PulseAudio) ###"
timeout 4 parecord --rate=16000 --channels=1 --format=s16le --raw > /tmp/pa.raw 2>/tmp/pa.err
echo "code=$? octets=$(stat -c %s /tmp/pa.raw 2>/dev/null)"
echo "erreur: $(head -c 300 /tmp/pa.err)"
echo
echo "### TEST 3 : parecord sur le monitor du sink (= son systeme) ###"
timeout 4 parecord --device=faux.monitor --rate=16000 --channels=1 --format=s16le --raw > /tmp/pa2.raw 2>/tmp/pa2.err
echo "code=$? octets=$(stat -c %s /tmp/pa2.raw 2>/dev/null)"
echo "erreur: $(head -c 300 /tmp/pa2.err)"
