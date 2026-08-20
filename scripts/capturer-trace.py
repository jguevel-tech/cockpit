#!/usr/bin/env python3
"""Capture la sortie BRUTE d'un programme dans un PTY de taille fixe (80x24).

Sert a nourrir le test d'aller-retour de l'emulateur de terminal
(src-tauri/src/terminal/ecran/) avec de VRAIES traces : c'est la seule source de cas
que personne n'aurait pense a ecrire. Les traces obtenues vivent dans
src-tauri/tests/traces/ et sont embarquees a la compilation du test.

Usage :
    scripts/capturer-trace.py <fichier-de-sortie> <secondes> <entrees-hex> -- cmd args...

`entrees-hex` : octets a envoyer au programme, en hexadecimal, groupes separes par des
virgules, un groupe toutes les 0,35 s (ex: "1b5b42,71" = fleche bas puis « q »).
Chaine vide pour n'envoyer rien.

Exemples reellement utilises :
    scripts/capturer-trace.py src-tauri/tests/traces/htop.raw 5 "" -- htop -d 5
    scripts/capturer-trace.py src-tauri/tests/traces/vim.raw 7 \
        "1b5b42,1b5b42,3a736574206e756d626572,0d" -- vim -u NONE -N +"syntax on" fichier.rs

La taille est FIXEE a 80x24 : le test rejoue les traces dans un ecran de cette taille,
une trace captee autrement ne veut rien dire.
"""
import fcntl, os, pty, select, signal, struct, sys, termios, time

sortie, duree, entrees = sys.argv[1], float(sys.argv[2]), sys.argv[3]
assert sys.argv[4] == "--"
cmd = sys.argv[5:]

pid, fd = pty.fork()
if pid == 0:
    env = dict(os.environ)
    env["TERM"] = "xterm-256color"
    env["COLUMNS"] = "80"
    env["LINES"] = "24"
    env["LANG"] = "en_US.UTF-8"
    # Fuites de l'AppImage : elles cassent python3 et curl dans les processus enfants
    # (voir « Pieges connus » du CLAUDE.md).
    env.pop("PYTHONHOME", None)
    env.pop("PYTHONPATH", None)
    env.pop("LD_LIBRARY_PATH", None)
    os.execvpe(cmd[0], cmd, env)

fcntl.ioctl(fd, termios.TIOCSWINSZ, struct.pack("HHHH", 24, 80, 0, 0))

groupes = [bytes.fromhex(g) for g in entrees.split(",") if g]
brut = bytearray()
debut = time.time()
prochaine = debut + 0.6
while time.time() - debut < duree:
    r, _, _ = select.select([fd], [], [], 0.1)
    if r:
        try:
            bloc = os.read(fd, 65536)
        except OSError:
            break
        if not bloc:
            break
        brut += bloc
    if groupes and time.time() >= prochaine:
        os.write(fd, groupes.pop(0))
        prochaine = time.time() + 0.35

try:
    os.kill(pid, signal.SIGKILL)
except ProcessLookupError:
    pass
os.waitpid(pid, os.WNOHANG)
os.close(fd)
open(sortie, "wb").write(bytes(brut))
print(f"{sortie}: {len(brut)} octets")
