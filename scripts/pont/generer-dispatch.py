"""Genere le dispatch du pont a partir des commandes lues dans lib.rs.

Ce qui n'est PAS traitable est SAUTE ET NOMME : une commande silencieusement absente
donnerait une interface qui s'affiche et ment, ce que le pont refuse deja par principe.
"""
import json, re, sys, io

cmds = json.load(open(sys.argv[1]))

def camel(s):
    tete, *reste = s.split('_')
    return tete + ''.join(m.capitalize() for m in reste)

lignes, sautees, portees, directes = [], [], [], []

for c in cmds:
    nom, args = c['nom'], c['args']
    # une reference dans les arguments ne se deserialise pas depuis du JSON
    if any('&' in a['type'] for a in args):
        sautees.append((nom, 'argument par reference')); continue
    if c['handle']:
        sautees.append((nom, 'demande un AppHandle')); continue
    # **TOUT TYPE DE TAURI EST UN REFUS.** Un `WebviewWindow` ne se deserialise pas depuis du
    # JSON : ne guetter que `AppHandle` laissait passer la fenetre, et l'erreur ne sortait
    # qu'a la compilation du fichier genere.
    # Il ne doit plus rester un seul argument venu d'une interface graphique : le backend
    # ne connait plus que l'etat et des donnees.
    etrangers = [x for x in args if any(
        m in x['type'] for m in ('tauri::', 'WebviewWindow', 'Window', 'Webview', 'State<'))]
    if etrangers:
        sautees.append((nom, f"argument lie a une interface : {etrangers[0]['type']}")); continue

    appel_args = ''.join(
        f'\n                serde_json::from_value(prendre(a, "{camel(x["nom"])}", "{x["nom"]}"))\n'
        f'                    .map_err(|e| format!("argument {camel(x["nom"])} : {{e}}"))?,'
        for x in args)

    if c['etat']:
        # L'etat est passe par le pont, jamais lu dans l'appel. Il n'y a plus de « facade »
        # a viser : la commande EST la fonction, depuis que les enveloppes de Tauri sont
        # parties.
        cible = f'{c.get("module", "crate::")}{nom}'
        tete = 'etat, ' if not args else 'etat,'
        portees.append(nom)
    else:
        cible = f'{c.get("module", "crate::")}{nom}'
        tete = ''
        directes.append(nom)

    attente = '.await' if c['async'] else ''
    interro = '?' if 'Result<' in c['retour'] else ''
    corps = f'{cible}({tete}{appel_args}\n            ){attente}{interro}'
    lignes.append(
        f'        "{nom}" => typer(async {{\n'
        f'            valeur(serde_json::to_value({corps}))\n'
        f'        }})\n'
        f'        .await,')

entete = '''//! Le dispatch des commandes du pont. **GENERE — ne pas editer a la main** :
//! `scripts/pont-dispatch.py` le reconstruit depuis les commandes de `lib.rs`, et une
//! retouche ici serait perdue au prochain passage.
//!
//! **LES ARGUMENTS ARRIVENT EN camelCase.** C'est la macro de Tauri qui les convertit
//! aujourd'hui ; sans cette conversion ici, toute commande a arguments echouerait alors
//! que la meme marche sous Tauri. Le nom en snake_case est accepte aussi, comme le fait
//! Tauri, pour qu'un appel ecrit a la main ne soit pas refuse sans raison lisible.

use crate::AppState;
use serde_json::Value;

/// Un argument, cherche sous ses deux noms. Absent, il vaut `null` : c'est `serde` qui
/// tranche ensuite, et un `Option<T>` l'accepte la ou un `T` le refuse avec un message
/// qui nomme le champ.
fn prendre(a: &Value, camel: &str, snake: &str) -> Value {
    a.get(camel).or_else(|| a.get(snake)).cloned().unwrap_or(Value::Null)
}

/// Donne au bloc de chaque branche le type que `?` reclame. Sans elle, le compilateur voit
/// une fonction qui rend `Option` et refuse tout `?` sur un `Result` : c'est ce qui a fait
/// echouer la premiere version generee.
fn typer<F: std::future::Future<Output = Result<Value, String>>>(f: F) -> F {
    f
}

/// Repond si la commande est connue, `None` sinon — le pont ajoute ses propres branches
/// et NOMME ce qui reste inconnu.
pub async fn appeler(
    etat: &AppState,
    commande: &str,
    a: &Value,
) -> Option<Result<Value, String>> {
    let valeur = |v: Result<Value, serde_json::Error>| v.map_err(|e| e.to_string());
    Some(match commande {
'''
pied = '''        _ => return None,
    })
}
'''
io.open(sys.argv[2], 'w', encoding='utf-8').write(entete + '\n'.join(lignes) + '\n' + pied)

print(f'branches generees : {len(lignes)}  (dont {len(portees)} via facade, {len(directes)} directes)')
print(f'sautees : {len(sautees)}')
for n, r in sautees: print(f'    {n:34} {r}')
json.dump({'portees': portees, 'directes': directes},
          open('/tmp/claude-1000/-home-jguevel-Documents-workspace-core-cockpit/a538524f-3d44-40c1-8fa1-3db12cc57bac/scratchpad/plan.json', 'w'))
