// Le journal de la coquille, ecrit dans le MEME fichier que celui du backend.
//
// **UNE ERREUR DE LA COQUILLE N'ALLAIT NULLE PART, ET CA S'EST VU CHEZ L'UTILISATEUR
// (2026-09-11).** Apres une mise a jour, une fenetre du systeme s'est affichee avant
// l'interface, puis a disparu. Rien n'en restait : ni dans `cockpit.log` (que seul le
// backend ecrivait), ni dans le journal de la session (l'application est lancee depuis un
// raccourci, sa sortie d'erreur ne va nulle part), ni dans Crashpad (il ne recueille que
// les crashs natifs, pas une exception JavaScript). Diagnostic impossible.
//
// Ecrire au meme endroit que le backend est deliberé : quand on cherche pourquoi un
// lancement s'est mal passe, on veut UNE chronologie, pas deux fichiers a recoller.
// L'ajout en fin de fichier depuis deux processus est sur ici : chaque ligne part en un
// seul `write`, et le journal en porte deja de plusieurs processus (l'application et le
// service de terminaux).

const fs = require('node:fs')
const path = require('node:path')
const os = require('node:os')

const NOM = 'cockpit.log'
// Le meme seuil que le backend (`report::LOG_MAX_BYTES`) : au-dela, le fichier est reparti
// dans un `.1`. Deux regles differentes feraient tourner le journal deux fois.
const MAX_OCTETS = 2 * 1024 * 1024

/**
 * Le dossier de donnees, calcule sans Electron.
 *
 * **VOLONTAIREMENT INDEPENDANT DE `app.getPath`** : ce module doit pouvoir ecrire une
 * exception survenue AVANT que l'application soit prete, ce qui est justement le moment ou
 * une panne de demarrage est la plus difficile a comprendre. La regle recopiee est celle du
 * backend (`chemins::calculer_le_dossier_de_donnees`), pour designer le meme dossier.
 */
function dossierDeDonnees() {
  const base = process.env.XDG_DATA_HOME || path.join(os.homedir(), '.local', 'share')
  return path.join(base, 'com.cockpit.dev')
}

/** « 2026-09-11 14:30:07 », en heure locale : le meme format que les lignes du backend. */
function horodatage() {
  const d = new Date()
  const p = (n) => String(n).padStart(2, '0')
  return (
    `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())} ` +
    `${p(d.getHours())}:${p(d.getMinutes())}:${p(d.getSeconds())}`
  )
}

/**
 * Ecrit une ligne dans le journal local.
 *
 * **LE JOURNAL NE DOIT JAMAIS FAIRE ECHOUER CE QU'IL OBSERVE** : un disque plein ou un
 * droit manquant est ignore, sinon l'instrument devient lui-meme la panne. C'est la meme
 * regle que du cote Rust.
 */
function journaliser(portee, message) {
  try {
    const chemin = path.join(dossierDeDonnees(), 'logs', NOM)
    fs.mkdirSync(path.dirname(chemin), { recursive: true })
    try {
      if (fs.statSync(chemin).size > MAX_OCTETS) fs.renameSync(chemin, `${chemin}.1`)
    } catch {
      // Pas encore de journal : il va etre cree juste apres.
    }
    // Sur une seule ligne, comme cote backend : un journal se lit avec `grep`.
    const plat = String(message).replace(/[\r\n]+/g, ' ')
    fs.appendFileSync(chemin, `${horodatage()} [${portee}] ${plat}\n`)
  } catch {
    // Voir plus haut : on n'a rien de mieux a faire, et surtout rien a casser.
  }
}

module.exports = { journaliser, dossierDeDonnees }
