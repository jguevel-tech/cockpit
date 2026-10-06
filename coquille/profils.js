// Les profils de fenetre : ce qu'un nom de profil designe, calcule sans Electron.
//
// **UNE FENETRE, UN PROFIL, UN DOSSIER.** Le profil par defaut (`null`) garde le dossier
// d'avant les profils ; un profil nomme vit sous `profils/<nom>`. Le backend applique la
// meme regle de son cote (`chemins::dossier_du_profil`) a partir de `COCKPIT_PROFIL`.

const fs = require('node:fs')
const path = require('node:path')

/** La meme regle que `chemins::valider_nom_de_profil` et `src/lib/utils/profils.ts`. */
const REGLE_DU_NOM = /^[a-z0-9][a-z0-9-]{0,31}$/

function validerNom(nom) {
  if (typeof nom !== 'string' || !REGLE_DU_NOM.test(nom)) {
    throw new Error(
      `nom de profil invalide « ${nom} » : 1 a 32 caracteres parmi a-z, 0-9 et -, sans - au debut`
    )
  }
  return nom
}

function dossierDuProfil(racine, nom) {
  return nom === null ? racine : path.join(racine, 'profils', validerNom(nom))
}

/** Le defaut en tete, puis les dossiers de `profils/` dont le nom est valide, tries. */
function listerProfils(racine) {
  let noms = []
  try {
    noms = fs
      .readdirSync(path.join(racine, 'profils'), { withFileTypes: true })
      .filter((entree) => entree.isDirectory() && REGLE_DU_NOM.test(entree.name))
      .map((entree) => entree.name)
      .sort()
  } catch (e) {
    // Pas de dossier `profils/` : personne n'a encore cree de profil. Toute autre erreur
    // (droits) doit remonter, sinon la liste mentirait.
    if (e.code !== 'ENOENT') throw e
  }
  return [null, ...noms]
}

/**
 * La session Chromium du profil. Le defaut garde la session par defaut : c'est elle qui
 * porte le `localStorage` d'avant les profils (langue, preferences d'interface).
 */
function partitionDuProfil(nom) {
  return nom === null ? undefined : `persist:profil-${validerNom(nom)}`
}

function titreDuProfil(nom) {
  return nom === null ? 'Cockpit' : `Cockpit — ${nom}`
}

/**
 * L'environnement du backend d'une fenetre.
 *
 * **UN PROFIL NOMME NE GARDE NI `COCKPIT_DB` NI `COCKPIT_TERMINAUX_SOCKET`.** Poses pour le
 * developpement, ils l'emportent sur le profil cote backend : deux fenetres partageraient la
 * meme base et le meme service sans que rien ne le montre.
 */
function environnementDuBackend(base, nom) {
  const env = { ...base }
  delete env.COCKPIT_PROFIL
  if (nom !== null) {
    env.COCKPIT_PROFIL = validerNom(nom)
    delete env.COCKPIT_DB
    delete env.COCKPIT_TERMINAUX_SOCKET
  }
  return env
}

module.exports = {
  REGLE_DU_NOM,
  validerNom,
  dossierDuProfil,
  listerProfils,
  partitionDuProfil,
  titreDuProfil,
  environnementDuBackend
}
