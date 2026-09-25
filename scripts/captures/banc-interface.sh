#!/bin/bash
# Banc d'interface : lance le paquet dans un ecran virtuel et capture des ecrans.
#
#   scripts/captures/banc-interface.sh <chemin/vers/l-AppImage> [dossier de sortie]
#
# **POURQUOI IL VIT DANS LE DEPOT.** Il a ete reecrit quatre fois dans `/tmp`, ou le systeme
# l'efface. Un banc qu'on rejoue a chaque correction d'interface merite d'etre range avec le
# reste, comme `prendre.sh` et `visite.sh` — dont il reprend le socle.
#
# Ce qu'il respecte, et qui n'est pas negociable (voir CLAUDE.md) :
#  - JAMAIS l'ecran reel : une fenetre de test y tomberait sur celle de l'utilisateur ;
#  - des dossiers XDG A LUI : un banc n'ecrit jamais dans le dossier de donnees de la machine ;
#  - un arret cible par une VARIABLE D'ENVIRONNEMENT posee sur ce lancement, jamais par le nom
#    du programme : le mainteneur fait tourner sa propre installation, qui porte le meme nom.
set -euo pipefail

# python-xlib et le reste vivent dans des prefixes a nous.
# Le fichier vit a cote du depot, avec les autres instructions de travail.
ENV_CONSTRUCTION="$(cd "$(dirname "$0")/../.." && pwd)/../.claude/env-construction.sh"
[ -f "$ENV_CONSTRUCTION" ] || { echo "env-construction.sh introuvable : $ENV_CONSTRUCTION" >&2; exit 1; }
# shellcheck disable=SC1090
source "$ENV_CONSTRUCTION"

BINAIRE="${1:?chemin de l AppImage attendu}"
# **UN BANC QUI LANCE UN PAQUET PERIME MENT SANS UN MOT.** Apres une release, le numero de
# version change : le paquet frais s'appelle autrement, et un chemin tape de memoire fait
# regarder l'AVANT-DERNIERE construction. Constate le 2026-09-17 : une demi-heure passee a
# chercher pourquoi une barre d'onglets ne s'affichait pas, alors qu'elle etait dans un fichier
# que l'on ne lancait pas. On refuse donc, sauf mention explicite.
DERNIER="$(ls -t "$(dirname "$BINAIRE")"/*.AppImage 2>/dev/null | head -1)"
if [ -n "$DERNIER" ] && [ "$DERNIER" != "$BINAIRE" ] && [ -z "${COCKPIT_BANC_VIEUX:-}" ]; then
  echo "ce paquet n'est pas le plus recent du dossier :" >&2
  echo "  demande : $BINAIRE" >&2
  echo "  le plus recent : $DERNIER" >&2
  echo "  (COCKPIT_BANC_VIEUX=1 pour l'ignorer)" >&2
  exit 2
fi
TRAVAIL="${2:-/tmp/cockpit-banc-interface}"
ECRAN=:93
ICI="$(cd "$(dirname "$0")" && pwd)"
OUTILS="$ICI/outils.py"

for m in "$TRAVAIL"/run/gvfs "$TRAVAIL"/run/doc; do
  [ -d "$m" ] && { fusermount -u "$m" 2>/dev/null || fusermount -uz "$m" 2>/dev/null; } || true
done
rm -rf "$TRAVAIL"
mkdir -p "$TRAVAIL/run" "$TRAVAIL/home/projets" "$TRAVAIL/img" "$TRAVAIL/bin"
# **SANS CE FICHIER, ZSH LANCE SON ASSISTANT DE PREMIERE CONFIGURATION** et mange la premiere
# touche envoyee au terminal : la commande du banc partait amputee de sa premiere lettre.
touch "$TRAVAIL/home/.zshrc" 
chmod 700 "$TRAVAIL/run"

# **UN VRAI CLUSTER SE PRETE, IL NE SE COPIE PAS.** Pour eprouver l'ecran Kubernetes, on
# pointe le kubeconfig de la machine SANS le recopier dans le dossier du banc : un jeton
# d'acces a une production n'a rien a faire dans /tmp. Vide, l'ecran dit qu'il n'a pas de
# cluster, ce qui est aussi un cas a regarder.
# **L'ECRAN KUBERNETES S'EPROUVE SUR UN FAUX CLUSTER, PAS SUR UNE PRODUCTION.** Un banc qui
# depend d'un vrai cluster ne se rejoue pas : le jour ou celui-ci a refuse toutes les requetes,
# kubectl compris, il n'y avait plus aucun moyen de regarder l'interface. `faux-cluster.py`
# sert ce dont l'ecran a besoin et ecrit son propre kubeconfig. `COCKPIT_BANC_KUBECONFIG`
# reste possible pour viser un vrai cluster a la main, mais rien n'en depend.
if [ -n "${COCKPIT_BANC_K8S:-}" ] && [ -z "${COCKPIT_BANC_KUBECONFIG:-}" ]; then
  python3 "$ICI/faux-cluster.py" 18443 "$TRAVAIL" > "$TRAVAIL/faux-cluster.out" 2>&1 &
  for _ in $(seq 20); do
    [ -s "$TRAVAIL/faux-cluster.out" ] && break
    sleep 0.3
  done
  COCKPIT_BANC_KUBECONFIG="$(head -1 "$TRAVAIL/faux-cluster.out")"
  echo "faux cluster : $COCKPIT_BANC_KUBECONFIG"
fi
export KUBECONFIG="${COCKPIT_BANC_KUBECONFIG:-}"
export XDG_RUNTIME_DIR="$TRAVAIL/run" XDG_DATA_HOME="$TRAVAIL/home/.local/share"
export XDG_CONFIG_HOME="$TRAVAIL/home/.config" XDG_CACHE_HOME="$TRAVAIL/home/.cache"
export HOME="$TRAVAIL/home" DISPLAY="$ECRAN" GIO_USE_VFS=local COCKPIT_LANGUE=fr
export COCKPIT_DB="$TRAVAIL/home/.local/share/com.cockpit.dev/cockpit.db"
export PATH="$TRAVAIL/bin:$PATH"
JETON="interface-$$"

arreter() { python3 "$OUTILS" arreter "COCKPIT_HARNAIS=$JETON" || true; }
trap 'arreter; kill %1 2>/dev/null || true' EXIT

# **UN FAUX AGENT, POUR EPROUVER CE QUI DEPEND D'UN AGENT.** Le service reconnait un agent au
# NOM du programme dans l'arbre de process : un script nomme `claude` suffit, et il evite de
# lancer le vrai (qui consommerait un abonnement et repondrait differemment a chaque fois).
# Il ecrit, puis se tait : c'est exactement « il attend une reponse ».
# **UN SCRIPT NE SUFFIT PAS : la detection lit `/proc/<pid>/exe`**, le vrai binaire, parce
# qu'argv[0] peut mentir (constate en 2026 sur un claude natif qui s'affichait sous un autre
# nom). Un `#!/bin/sh` donne donc `exe = /bin/sh` et passe inapercu. On COPIE donc un shell
# sous le nom `claude` : son chemin porte alors le nom que la detection cherche.
cp /bin/sh "$TRAVAIL/bin/claude"
cat > "$TRAVAIL/bin/faux-agent" <<'AGENT'
echo "je travaille"
sleep 2
echo "j ai fini d ecrire, j attends"
sleep 600
AGENT

Xvfb "$ECRAN" -screen 0 1680x1050x24 -nolisten tcp >/dev/null 2>&1 &
sleep 2
lancer() { COCKPIT_HARNAIS="$JETON" dbus-run-session -- "$BINAIRE" >>"$TRAVAIL/app.log" 2>&1 & sleep 20; }
clic()   { python3 "$OUTILS" cliquer "$1" "$2"; sleep "${3:-2}"; }
image()  { python3 "$OUTILS" capturer "$TRAVAIL/img/$1.png" >/dev/null; }

lancer; arreter; sleep 2
python3 "$OUTILS" preparer "$COCKPIT_DB" "$TRAVAIL/home/projets" fr
P="$TRAVAIL/home/projets/boutique-vinyles"
# `preparer` inscrit les projets en base mais ne cree pas leurs dossiers.
mkdir -p "$P"
git -C "$P" init -q -b main 2>/dev/null || true
git -C "$P" config user.email banc@exemple.test; git -C "$P" config user.name Banc
echo "demo" > "$P/README.md"; git -C "$P" add -A; git -C "$P" commit -qm "Poser les bases"

lancer
clic 105 296 3            # le projet « boutique-vinyles » (le premier de la liste)
clic 727 127 10           # l onglet Terminal
image 1-avant
# **LES LIENS DU PROJET TIENNENT DANS UN MENU**, sur la ligne des onglets : sept liens cotes a
# cote prenaient une ligne entiere de l'ecran.
clic 1190 176 2           # le bouton « Liens »
image 1b-liens

# **UN COLLAGE VA DANS LE TERMINAL QU'ON VISE, PAS DANS CELUI D'AVANT.** Signale le 2026-09-24 :
# on copie dans le terminal de l'agent, on passe sur un autre onglet, on colle, et le texte
# arrive chez l'agent. Il faut que les terminaux aient ete crees par un affichage PRECEDENT de
# l'onglet : on en ouvre deux, on revient sur le premier, on part sur Git et on revient.
if [ -n "${COCKPIT_BANC_COLLER:-}" ]; then
  clic 900 500 1          # ferme le menu des liens
  clic 900 500 1          # focalise le premier terminal
  python3 "$OUTILS" taper "echo terminal-UN"
  clic 509 221 4          # « + » : le second terminal
  python3 "$OUTILS" taper "echo terminal-DEUX"
  image coller-1-deux-terminaux
  clic 400 221 3          # retour sur le premier
  clic 862 127 3          # Git, puis retour : l'onglet des terminaux est remonte
  clic 727 127 5
  image coller-2-retour
  clic "${COCKPIT_BANC_ONGLET2_X:-580}" 221 3   # l'onglet du second terminal
  printf 'MARQUE-COLLEE' | DISPLAY="$ECRAN" xclip -selection clipboard
  python3 "$OUTILS" cliquer 900 500 2           # clic molette dans le second terminal
  sleep 2
  image coller-3-apres-molette
  clic 400 221 3          # le premier : le texte ne doit PAS y etre
  image coller-4-premier
  # **LE CLIC DROIT GARDE LA SELECTION**, y compris quand le programme suit la souris, comme
  # claude : on l'active a la main, on selectionne en tenant Maj, puis clic droit.
  printf '#!/bin/bash\nprintf "\\e[?1000h\\e[?1006h"\necho mot-a-copier\nod -c\n' > "$TRAVAIL/bin/souris"
  python3 "$OUTILS" taper "clear"
  python3 "$OUTILS" taper "bash $TRAVAIL/bin/souris"
  sleep 1
  python3 "$OUTILS" glisser 332 273 440 273 --maj
  sleep 1
  image coller-5-selection
  python3 "$OUTILS" cliquer 380 273 3
  sleep 1
  image coller-6-clic-droit
  if [ -n "${COCKPIT_BANC_SOURIS:-}" ]; then
    python3 "$OUTILS" cliquer 900 700 1; sleep 1
    python3 "$OUTILS" cliquer 900 700 1; sleep 1
    image coller-7-clic-gauche
  fi
  echo "images collage : $TRAVAIL/img"
  exit 0
fi

# **LE CURSEUR REVIENT QUAND LE PROGRAMME QUI L'AVAIT MASQUE N'EST PLUS LA.** Signale le
# 2026-09-25 apres un manque de memoire qui a tout arrete : les terminaux sont revenus sans
# curseur. Un programme comme claude masque le curseur ; s'il meurt sans le remettre, plus
# personne ne le remet. Deux cas : le programme seul est tue, puis tout le service.
if [ -n "${COCKPIT_BANC_CURSEUR:-}" ]; then
  printf '#!/bin/bash\nprintf "\\e[?25l"\necho curseur-masque\necho $$ > %s/masquer.pid\nexec sleep 600\n' "$TRAVAIL" > "$TRAVAIL/bin/masquer"
  clic 900 500 1
  clic 900 500 1
  python3 "$OUTILS" taper "bash $TRAVAIL/bin/masquer"
  sleep 2
  image curseur-1-masque
  kill "$(cat "$TRAVAIL/masquer.pid")" || true   # le programme seul meurt, le shell reprend la main
  sleep 8                 # la liste des terminaux passe toutes les cinq secondes
  image curseur-2-programme-mort
  python3 "$OUTILS" taper "bash $TRAVAIL/bin/masquer"
  sleep 2
  # Le service tue d'un coup, comme systemd l'a fait : SIGKILL, sans photo.
  for pid in $(pgrep -f -- "--service-terminaux"); do
    if tr '\0' '\n' < /proc/$pid/environ 2>/dev/null | grep -q "COCKPIT_HARNAIS=$JETON"; then
      kill -9 $pid; echo "  service $pid tue"
    fi
  done
  kill "$(cat "$TRAVAIL/masquer.pid")" 2>/dev/null || true
  sleep 3
  clic 862 127 3; clic 727 127 8    # Git puis Terminal : l'onglet se rouvre
  clic 900 500 3
  image curseur-3-apres-le-service
  echo "images curseur : $TRAVAIL/img"
  exit 0
fi

if [ -n "${COCKPIT_BANC_NS:-}" ]; then
  # **LE NAMESPACE CHOISI DOIT REVENIR.** Signale par le mainteneur : il choisit celui de son
  # projet, part, revient, et retrouve celui du contexte. On pose le choix EN BASE, comme s'il
  # datait de la session precedente : c'est la RELECTURE qu'on eprouve, pas le clic.
  python3 - "$COCKPIT_DB" <<'SQL'
import json, sqlite3, sys
c = sqlite3.connect(sys.argv[1])
c.execute("insert or replace into settings(key, value) values(?, ?)",
          ("k8s.cible.api-facturation",
           json.dumps({"contexte": "cluster-prod-02",
                       "namespace": "core-akamai-logs-ccmbg-com-main"})))
c.commit()
print("reglage pose")
SQL
  clic 941 127 15         # l onglet Kubernetes
  image ns-1-relecture
  clic 862 127 4          # on part sur Git
  clic 941 127 12         # et on revient
  image ns-2-retour
  echo "images namespace : $TRAVAIL/img"
  exit 0
fi

if [ -n "${COCKPIT_BANC_MAJ:-}" ]; then
  # La cloche des mises a jour : elle doit voir la derniere Release publiee.
  clic 1063 55 3
  image maj-1-cloche
  clic 1342 101 15        # « Verifier »
  image maj-2-verifie
  echo "images mise a jour : $TRAVAIL/img"
  exit 0
fi

# **LE THEME CLAIR SE REGARDE AUSSI.** Le fond des graphiques et les couleurs des courbes y
# repondent autrement : un contraste correct en sombre ne prouve rien. Le theme vit dans le
# stockage de la page, donc on le choisit en cliquant, comme un utilisateur.
if [ -n "${COCKPIT_BANC_CLAIR:-}" ]; then
  clic 1275 55 3          # l engrenage
  clic 400 215 3          # « Apparence »
  clic 1117 232 3         # la palette « Clair »
  image clair-1-apparence
  clic 105 296 3          # un projet
  clic 941 127 14         # l onglet Kubernetes
  clic 849 268 3          # l onglet « Ressources »
  sleep 30                # de quoi remplir les courbes
  image clair-2-ressources
  # **UNE BANDE SANS NOM NE SERT A RIEN** : le survol doit dire QUEL pod on designe.
  python3 "$OUTILS" survoler 600 540
  image clair-3-survol
  exit 0
fi

# **UN ECRAN QUI A TROIS HEURES D'HISTORIQUE NE SE SIMULE PAS EN TROIS MINUTES.** Les courbes
# se sont montrees VIDES chez l'utilisateur avec 21 h de mesures enregistrees, alors qu'elles se
# remplissaient au banc apres deux minutes de direct. On ecrit donc l'historique en base, comme
# s'il avait ete mesure toute la matinee, et on regarde.
if [ -n "${COCKPIT_BANC_K8S_HISTO:-}" ]; then
  python3 - "$COCKPIT_DB" <<'SQL'
import json, sqlite3, sys, time
c = sqlite3.connect(sys.argv[1])
maintenant = int(time.time() * 1000)
# Le banc ouvre le PREMIER projet de la liste : la cible se pose sur celui-la, sinon la
# periode retenue n'est pas relue et l'ecran repart sur cinq minutes.
for projet in ("boutique-vinyles", "api-facturation"):
    c.execute("insert or replace into settings(key, value) values(?, ?)",
              (f"k8s.cible.{projet}",
               json.dumps({"contexte": "cluster-demo", "namespace": "equipe-demo",
                           "fenetre": 10800, "rythme": 60})))
c.execute("insert or replace into settings(key, value) values(?, ?)",
          ("k8s.surveillance",
           json.dumps([{"contexte": "cluster-demo", "namespace": "equipe-demo",
                        "periode": 300, "actif": True}])))
pods = [(f"web-5cb5677dcc-{i}xj4{i}", 250 + i * 20) for i in range(4)]
pods += [(f"post-create-alpha-consumer-5cb5677dcc-{i}", 0) for i in range(20)]
for pod, _ in pods:
    c.execute("insert or ignore into k8s_cibles (contexte, namespace, pod) values (?, ?, ?)",
              ("cluster-demo", "equipe-demo", pod))
ids = {p: c.execute("select id from k8s_cibles where pod = ?", (p,)).fetchone()[0]
       for p, _ in pods}
# 21 h d'historique, comme sur l'ecran ou le defaut a ete vu : c'est le VOLUME qu'on cherche
# a reproduire, pas seulement la fenetre affichee.
lignes = []
for minute in range(21 * 60, 0, -1):
    t = maintenant - minute * 60_000
    for pod, cpu in pods:
        lignes.append((ids[pod], t, cpu, 120 * 1024 * 1024))
c.executemany("insert or replace into k8s_mesures (cible, t, cpu, ram) values (?, ?, ?, ?)",
              lignes)
c.commit()
print(f"  historique pose : {len(lignes)} points, {len(pods)} pods, 21 h")
SQL
  # **L'IMAGE DE FOND CHANGE LE RENDU, ET C'EST LA DIFFERENCE QU'ON CHERCHE.** Les surfaces
  # deviennent translucides ; un ecran correct en theme uni ne prouve rien (regle du projet).
  # Le fond est un simple fichier dans le dossier de donnees : on le depose avant de lancer.
  if [ -n "${COCKPIT_BANC_FOND:-}" ]; then
    mkdir -p "$TRAVAIL/home/.local/share/com.cockpit.dev"
    cp "$(cd "$ICI/../.." && pwd)/docs/captures/terminal.png" \
       "$TRAVAIL/home/.local/share/com.cockpit.dev/wallpaper.png"
    echo "  image de fond posee"
  fi
  clic 105 296 3          # le projet
  clic 941 127 14         # l onglet Kubernetes
  clic 849 268 8          # l onglet « Ressources »
  image histo-1-courbes
  # **CLIQUER UNE BANDE SUIT CE POD, SUR LES DEUX GRAPHIQUES ET DANS LE CLASSEMENT.** Sans ca,
  # il fallait retrouver dans la liste du dessous le pod qu'on venait de montrer du doigt.
  python3 "$OUTILS" survoler 600 540
  clic 600 540 4
  image histo-2-suivi
  clic 1126 307 4         # « Reinitialiser », entre la periode et la mesure en direct
  image histo-3-remis
  echo "images historique : $TRAVAIL/img"
  exit 0
fi

if [ -n "${COCKPIT_BANC_K8S:-}" ]; then
  clic 941 127 1          # ferme le menu des liens, reste ouvert apres sa capture
  clic 941 127 14         # l onglet Kubernetes (a droite de Git)
  image k8s-1-ensemble    # ce que le namespace contient, en objets declares
  # **LA VUE DES TACHES PLANIFIEES EST CELLE QUI A MOTIVE TOUT CECI.** Elle doit montrer les
  # taches DECLAREES, y compris celles qui ne se sont jamais declenchees et n'ont donc aucun pod.
  clic 632 268 3          # l onglet « Taches planifiees »
  image k8s-2-taches
  # **LE MENAGE DE TOUT LE NAMESPACE EN UN GESTE**, a droite des onglets de vue.
  if [ -n "${COCKPIT_BANC_TOUT_NETTOYER:-}" ]; then
    clic "${COCKPIT_BANC_TOUT_X:-1300}" 268 3
    image k8s-tout-1-confirmation
    clic 884 504 12       # « Supprimer »
    image k8s-tout-2-nettoye
    exit 0
  fi
  # **SUPPRIMER LES PODS D'UN TRAVAIL QUI A RATE**, ce qu'on vient faire apres avoir corrige.
  clic 600 320 4          # « nettoyage-archives », en tete car il porte quatre echecs
  image k8s-2b-echecs
  clic 1270 391 3         # « Supprimer les 4 pods en echec »
  image k8s-2c-confirmation
  clic 884 504 12         # « Supprimer » : le flux annonce ensuite les quatre disparitions
  image k8s-2d-nettoye
  clic 380 317 3          # retour a la liste
  clic 600 400 3          # une tache de la liste : on entre dedans
  image k8s-3-dans-la-tache
  # Un pod ouvert DEPUIS un objet doit rendre le meme detail que depuis la liste des pods.
  clic 500 389 4
  image k8s-3b-pod-depuis-la-tache
  clic 1351 350 1         # fermer le detail
  clic 380 317 3          # « Retour a la liste »
  image k8s-4-retour
  clic 500 268 3          # l onglet « Services »
  image k8s-5-services
  clic 742 268 3          # l onglet « Pods »
  image k8s-6-pods
  clic 1000 228 1         # le champ de recherche
  python3 "$OUTILS" taper "web" 2>/dev/null || true
  sleep 3
  image k8s-7-recherche
  # **UNE RECHERCHE QUI NE TROUVE RIEN ICI DOIT DIRE OU ELLE TROUVE.** « snap » ne rend aucun
  # pod : la tache planifiee de ce nom ne s'est jamais declenchee. L'ecran doit y emmener.
  clic 1000 228 1
  python3 "$OUTILS" effacer 2>/dev/null || true
  python3 "$OUTILS" taper "snap" 2>/dev/null || true
  sleep 2
  image k8s-7b-passerelle
  clic 1000 228 1
  python3 "$OUTILS" effacer 2>/dev/null || true
  python3 "$OUTILS" taper "web" 2>/dev/null || true
  sleep 2
  clic 500 376 6          # le premier pod trouve : ouvre son detail et ses logs
  image k8s-8-detail
  sleep 6                 # de quoi voir arriver des lignes en direct
  image k8s-9-logs
  clic 1351 311 1         # fermer le detail
  clic 1000 228 1
  python3 "$OUTILS" effacer 2>/dev/null || true
  clic 849 268 3          # l onglet « Ressources »
  sleep 25                # cinq mesures a cinq secondes
  image k8s-10-ressources
  clic 600 620 4          # le premier pod du classement : on le suit de pres
  image k8s-11-focus
  # Les reglages : c'est la qu'on declare ce que Cockpit suit en continu.
  # On enregistre le namespace affiche AVANT d'aller aux reglages : sans cible, l'ecran des
  # reglages ne montre ni le rythme ni le cout par jour, donc rien de ce qu'on veut relire.
  clic 1264 368 4         # « Enregistrer ce namespace », a droite du bandeau
  image k8s-11b-enregistre
  # **ENREGISTRER UN SECOND NAMESPACE EST LE GESTE QUI ECHOUAIT** (« An object could not be
  # cloned ») : la liste envoyee portait alors un objet venu de l'etat, que le pont refuse de
  # cloner. Le premier passait, donc le defaut ne se voyait qu'une fois l'ecran rempli.
  clic 610 228 2          # le selecteur de namespace
  clic 450 364 10         # « boutique-demo », la premiere entree SOUS le champ de filtre
  clic 849 268 5          # l onglet « Ressources »
  image k8s-11c-avant     # ou est le bouton a cet instant, pour recaler le clic
  clic 1264 368 6         # « Enregistrer ce namespace », pour le second
  image k8s-11c-second
  clic 1275 55 3          # l engrenage
  clic 380 300 3          # l entree « Kubernetes » du menu
  image k8s-12-reglages
  echo "images kubernetes : $TRAVAIL/img"
  exit 0
fi

# On lance le faux agent dans le terminal affiche : ce qu'on verifie ici, c'est qu'un agent
# tourne bien dans un terminal et que la barre laterale le SIGNALE (l'asterisque). Les reperes
# « attend » et « fini » ont ete retires en 0.87.0, il n'y a plus rien a en attendre.
clic 900 500 1
python3 "$OUTILS" taper "claude $TRAVAIL/bin/faux-agent"
sleep 5
image 2-agent-en-cours

# **L'APPLICATION SE RELANCE SANS PERDRE SES TERMINAUX**, le service survit : on arrete sans y
# toucher, on relance, et le terminal doit revenir avec ce qu'il affichait.
python3 "$OUTILS" arreter "COCKPIT_HARNAIS=$JETON" --sauf-service
sleep 3
lancer
# **PENDANT QU'IL SE ROUVRE, L'ECRAN DOIT LE DIRE.** Il annoncait « aucun terminal ouvert » et
# proposait d'en creer un, alors qu'il en rechargeait un : on croyait tout perdu, et cliquer en
# aurait cree un de plus. On capture donc TOT, avant la fin du chargement.
clic 105 296 2            # le projet
clic 727 127 0            # l onglet Terminal, sans laisser le temps de finir
image 3-pendant-la-reouverture
sleep 8
image 4-apres-relance

# **UN TERMINAL OUVERT PAR « + » DOIT RESTER CELUI QU'ON VOIT.** Signale le 2026-09-23 :
# « parfois ça revient sur l'autre terminal ». Le premier terminal porte l'ecran du faux
# agent, reconnaissable ; on en ouvre un second, et l'onglet actif doit montrer une invite
# NEUVE, pas l'agent. Puis on provoque ce qui remettait l'ancien : une nouvelle mise en page.
clic 862 127 3            # on part sur Git
clic 727 127 0            # retour sur Terminal, sans attendre la fin du montage
clic 594 224 0            # « + » tout de suite
sleep 4
image 5-apres-plus
python3 "$OUTILS" survoler 900 500
clic 797 127 3            # Fichiers, puis retour : la disposition est relue
clic 727 127 4
image 6-apres-aller-retour
# Le second chemin : un simple CLIC sur l'autre onglet doit lui aussi survivre au retour.
clic 485 224 3            # l onglet « - 1 »
clic 862 127 3            # Git, puis retour
clic 727 127 4
image 7-clic-puis-retour

echo "images : $TRAVAIL/img"
grep -icE "error|erreur" "$TRAVAIL/app.log" | sed 's/^/  lignes d erreur dans le log : /'
