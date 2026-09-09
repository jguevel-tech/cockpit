"""Lit les commandes Tauri sans rien modifier. Le decoupage compte la PROFONDEUR des
delimiteurs : une regex sur `)` tombe au milieu de `Result<Vec<T>, String>`, et c'est
exactement ce qui a casse lib.rs au premier essai."""
import re, io, sys, json, collections

def fermeture(s, i, ouvrant='(', fermant=')'):
    prof = 0
    while i < len(s):
        if s[i] == ouvrant: prof += 1
        elif s[i] == fermant:
            prof -= 1
            if prof == 0: return i
        i += 1
    raise ValueError('delimiteur non ferme')

def decouper(args):
    """Decoupe une liste d'arguments aux virgules de PROFONDEUR ZERO."""
    morceaux, prof, courant = [], 0, ''
    for c in args:
        if c in '(<[': prof += 1
        elif c in ')>]': prof -= 1
        if c == ',' and prof == 0:
            morceaux.append(courant.strip()); courant = ''
        else:
            courant += c
    if courant.strip(): morceaux.append(courant.strip())
    return morceaux

def analyser(chemin):
    s = io.open(chemin, encoding='utf-8').read()
    trouvees = []
    # **DEUX PIEGES DANS CE SEUL MOTIF, PAYES CHACUN UNE FOIS.** L'attribut peut etre
    # conditionne (`cfg_attr`) : ne reconnaitre que la forme nue faisait disparaitre 72
    # commandes du dispatch SANS AUCUN SIGNAL, le generateur annoncant juste un total plus
    # petit. Et un commentaire de doc ou un `#[cfg]` se glisse entre l'attribut et le `fn` :
    # deux commandes sur 165 avaient ete manquees comme ca. Le controle qui attrape les deux,
    # c'est de compter les attributs presents dans les fichiers et de comparer.
    for m in re.finditer(
        r'#\[(?:tauri::command|cfg_attr\(feature = "interface-tauri", tauri::command\))\]\s*\n'
        r'(?:\s*(?://[^\n]*|#\[[^\]]*\])\n)*'
        r'\s*((?:pub\s+)?(async\s+)?fn\s+(\w+)\s*)\(', s):
        nom = m.group(3)
        ouvre = s.index('(', m.end(1) - 1)
        ferme = fermeture(s, ouvre)
        args = decouper(s[ouvre + 1:ferme])
        reste = s[ferme + 1:]
        accolade = reste.index('{')
        retour = reste[:accolade].strip()
        etat = [a for a in args if 'State<' in a]
        handle = [a for a in args if 'AppHandle' in a]
        autres = [a for a in args if a not in etat and a not in handle]
        trouvees.append({
            'nom': nom, 'async': bool(m.group(2)), 'retour': retour,
            'etat': bool(etat), 'handle': bool(handle),
            'args': [{'nom': a.split(':')[0].strip(), 'type': a.split(':', 1)[1].strip()} for a in autres],
        })
    return trouvees

# **LE PONT DOIT AUSSI SERVIR CE QUI N'EST PAS DANS lib.rs.** Les commandes du compte et
# de la synchro vivent ailleurs : ne lire que lib.rs les rendait injoignables hors Tauri,
# donc l'onglet compte mort sous la coquille, sans qu'aucune erreur ne le signale.
SOURCES = [
    ('src-tauri/src/lib.rs', 'crate::'),
    ('src-tauri/src/compte/mod.rs', 'crate::compte::'),
    ('src-tauri/src/compte/synchro.rs', 'crate::compte::synchro::'),
]
def compter_les_attributs(chemin):
    """Combien d'attributs de commande ce fichier porte VRAIMENT.

    **C'est le seul controle qui attrape un motif troue.** Un analyseur qui s'audite sur son
    propre motif est toujours vert : le 2026-09-09 il a rendu 111 commandes sur 183 sans
    lever le moindre signal, parce qu'il ne reconnaissait qu'une des deux formes d'attribut.
    Compter puis COMPARER est ce qui l'a revele."""
    t = io.open(chemin, encoding='utf-8').read()
    return len(re.findall(r'#\[tauri::command\]', t)) + len(re.findall(r'tauri::command\)\]', t))

cmds = []
for chemin, prefixe in SOURCES:
    lues = analyser(chemin)
    for x in lues:
        x['module'] = prefixe
    attendu = compter_les_attributs(chemin)
    if len(lues) != attendu:
        raise SystemExit(
            f'{chemin} : {len(lues)} commandes lues pour {attendu} attributs presents. '
            f'Le motif a un trou — corrige-le, ne contourne pas ce controle.'
        )
    cmds += lues
c = collections.Counter()
for x in cmds:
    if x['handle']: c['avec AppHandle (a traiter a part)'] += 1
    elif x['etat'] and not x['args']: c['State seul, sans argument'] += 1
    elif x['etat']: c['State + arguments'] += 1
    elif not x['args']: c['sans etat ni argument'] += 1
    else: c['arguments seuls'] += 1
print('commandes lues :', len(cmds))
for k, v in c.most_common(): print(f'  {v:4}  {k}')
json.dump(cmds, open(sys.argv[1], 'w'), indent=1)
