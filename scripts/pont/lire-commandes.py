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
    for m in re.finditer(r'#\[tauri::command\]\s*\n\s*((?:pub\s+)?(async\s+)?fn\s+(\w+)\s*)\(', s):
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

cmds = analyser('src-tauri/src/lib.rs')
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
