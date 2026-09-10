// Servir les commandes du plugin updater de Tauri, avec electron-updater derriere.
//
// **POURQUOI IMITER CE PLUGIN PLUTOT QUE REECRIRE L'INTERFACE.** `stores/update.ts` gere
// deja tout : les phases, la barre de progression, les messages d'erreur traduits, la notice
// de la cloche. Le remplacer serait reecrire du code eprouve pour rien. Les quatre commandes
// qu'il appelle sont servies ici, avec la meme forme de reponse.
//
// **LA BASCULE DEPUIS TAURI SE JOUE ICI.** Une installation qui recoit la version Electron
// sans cet updater ne se met plus JAMAIS a jour, et rien ne permet de la rattraper a
// distance. C'est le seul geste irreversible de toute la migration.

/**
 * `electron-updater` n'est charge qu'au PREMIER usage, jamais a l'import.
 *
 * **CHARGE EN TETE DE MODULE, IL EMPECHE L'APPLICATION DE DEMARRER.** Il lit la version de
 * l'application des sa construction, donc avant qu'Electron soit pret : le processus
 * principal s'arretait sans ouvrir de fenetre, sans une ligne de journal et sans code
 * d'erreur lisible. Un demarrage muet est le pire des etats — ici il a coute une
 * reconstruction complete du paquet pour etre nomme.
 */
let cache = null
function updater() {
  if (cache) return cache
  const { autoUpdater } = require('electron-updater')
  // Il telecharge tout seul des qu'il trouve quelque chose ; ici c'est l'interface qui
  // decide, comme sous Tauri.
  autoUpdater.autoDownload = false
  autoUpdater.autoInstallOnAppQuit = false
  // **SON JOURNAL EST COUPE, ET LES ERREURS NE SONT PAS PERDUES POUR AUTANT.** Il ecrit ses
  // pannes sur la console avant de les propager : trente lignes de trace au lancement pour
  // une situation normale (aucune Release ne porte encore de manifeste a son format). Ce
  // qui doit etre vu remonte par le canal de l'interface, ou `stores/update.ts` le nomme et
  // le traduit — le meme chemin que sous Tauri.
  autoUpdater.logger = null
  cache = autoUpdater
  return cache
}

/** La mise a jour trouvee par la derniere verification, gardee entre check et download. */
let trouvee = null

/**
 * Traduit la progression d'electron-updater vers les evenements du plugin Tauri.
 *
 * Les trois formes attendues par l'interface sont `Started` avec la taille totale,
 * `Progress` avec la taille du morceau, et `Finished`. Se tromper de forme ne leve aucune
 * erreur : la barre reste simplement a zero, ce qui se lit comme un telechargement bloque.
 */
function suivreLeTelechargement(pousser) {
  let annonce = false
  const surProgres = (p) => {
    if (!annonce) {
      annonce = true
      pousser({ event: 'Started', data: { contentLength: p.total } })
    }
    pousser({ event: 'Progress', data: { chunkLength: p.delta } })
  }
  const surFin = () => pousser({ event: 'Finished' })
  updater().on('download-progress', surProgres)
  updater().once('update-downloaded', surFin)
  return () => {
    updater().off('download-progress', surProgres)
    updater().off('update-downloaded', surFin)
  }
}

/**
 * Repond a une commande du plugin, ou rend `{ traite: false }` si ce n'en est pas une.
 *
 * `pousser` envoie un message a un canal de la page, par son identifiant.
 */
async function traiterUneCommandeDeMiseAJour(commande, arguments_, pousser) {
  switch (commande) {
    case 'plugin:updater|check': {
      let resultat
      try {
        resultat = await updater().checkForUpdates()
      } catch (e) {
        // **UN MANIFESTE ABSENT N'EST PAS UNE PANNE, C'EST « RIEN DE NEUF ».** Tant que la
        // derniere Release ne porte pas de manifeste au format d'electron-updater — le cas
        // de toutes celles construites par Tauri — la recherche rend un 404. Le laisser
        // remonter affichait une trace de trente lignes a chaque demarrage et faisait
        // passer une situation NORMALE pour une erreur.
        // Tout autre echec (reseau, jeton, release corrompue) remonte, lui : le magasin de
        // l'interface sait les nommer.
        if (e?.code === 'ERR_UPDATER_CHANNEL_FILE_NOT_FOUND') {
          trouvee = null
          return { traite: true, valeur: null }
        }
        throw e
      }
      const info = resultat?.updateInfo
      // Pas de version plus recente : `null`, et c'est ce que l'interface teste.
      if (!info || info.version === updater().currentVersion.version) {
        trouvee = null
        return { traite: true, valeur: null }
      }
      trouvee = info
      return {
        traite: true,
        valeur: {
          rid: 1,
          currentVersion: updater().currentVersion.version,
          version: info.version,
          date: info.releaseDate ?? null,
          // Les notes de version viennent du CHANGELOG, publiees dans la Release.
          body: typeof info.releaseNotes === 'string' ? info.releaseNotes : null,
          rawJson: {}
        }
      }
    }
    case 'plugin:updater|download':
    case 'plugin:updater|download_and_install': {
      if (!trouvee) throw new Error('aucune mise a jour a telecharger')
      const canal = arguments_?.onEvent
      const detacher = suivreLeTelechargement((message) => {
        if (typeof canal === 'string' && canal.startsWith('__CHANNEL__:')) {
          pousser(Number(canal.slice('__CHANNEL__:'.length)), message)
        }
      })
      try {
        await updater().downloadUpdate()
      } finally {
        detacher()
      }
      if (commande === 'plugin:updater|download_and_install') {
        updater().quitAndInstall()
      }
      return { traite: true, valeur: null }
    }
    case 'plugin:updater|install': {
      // Ne rend jamais la main : l'application se ferme pour se remplacer.
      updater().quitAndInstall()
      return { traite: true, valeur: null }
    }
    default:
      return { traite: false }
  }
}

module.exports = { traiterUneCommandeDeMiseAJour }
