// Le SEUL point de contact entre la page et le reste du monde.
//
// La page tourne dans le bac a sable : ni Node, ni acces au processus principal. Elle ne
// peut faire que ce qui est expose ici, nommement. C'est la porte d'Electron, il n'y en a
// pas d'autre — et depuis la 0.59.5 il n'y a plus que la sienne : l'imitation de l'API
// interne de Tauri (`__TAURI_INTERNALS__`, ses identifiants de rappel, ses commandes
// `plugin:*`) a ete retiree.

const { contextBridge, ipcRenderer, webUtils } = require('electron')

/** Les ecouteurs de la page, par nom d'evenement. */
const ecouteurs = new Map()

// Le processus principal pousse ici tout ce que le backend emet. La page n'est pas
// interrogee, elle est servie : aucun aller-retour pour s'abonner, aucun identifiant a
// faire circuler.
ipcRenderer.on('cockpit:evenement', (_evenement, nom, charge) => {
  const pour = ecouteurs.get(nom)
  if (!pour) return
  // Copie avant de parcourir : un ecouteur a le droit de se detacher en repondant.
  for (const fonction of [...pour]) fonction(charge)
})

contextBridge.exposeInMainWorld('cockpit', {
  /** Appelle une commande : celles de la coquille (`coquille:*`) et celles du backend. */
  invoke: (commande, arguments_ = {}) =>
    ipcRenderer.invoke('cockpit:commande', commande, arguments_),

  /**
   * Ecoute un evenement, et rend de quoi s'en detacher.
   *
   * **LE PROCESSUS PRINCIPAL N'ENVOIE QUE CE QUI EST ECOUTE.** Une surveillance qui tourne
   * sur un minuteur emet meme quand personne ne regarde : sans ce filtre, chaque passage
   * traverserait le pont pour rien. Le premier ecouteur d'un nom l'annonce, le dernier a
   * partir le retire.
   */
  surEvenement: (nom, fonction) => {
    let pour = ecouteurs.get(nom)
    if (!pour) {
      pour = new Set()
      ecouteurs.set(nom, pour)
      ipcRenderer.send('cockpit:ecouter', nom)
    }
    pour.add(fonction)
    return () => {
      const encore = ecouteurs.get(nom)
      if (!encore) return
      encore.delete(fonction)
      if (encore.size === 0) {
        ecouteurs.delete(nom)
        ipcRenderer.send('cockpit:ignorer', nom)
      }
    }
  },

  /** Une ressource locale, servie par notre schema. Aucun fichier hors de dist/ n'est lisible. */
  convertirChemin: (chemin) => `cockpit://interface/${String(chemin).replace(/^\/+/, '')}`,

  /**
   * Le chemin d'un fichier depose dans la fenetre.
   *
   * **`File.path` N'EXISTE PLUS DEPUIS ELECTRON 32**, et c'est ce qui a fait disparaitre le
   * glisser-deposer dans les terminaux en 0.59.0 : l'interface le demandait a Tauri, qui
   * n'est plus la. Seul le preload peut repondre.
   */
  cheminDuFichier: (fichier) => webUtils.getPathForFile(fichier)
})

// **LES PREFERENCES DE L'ANCIENNE VERSION, POSEES AVANT QUE LA PAGE NE DEMARRE.** Elles
// vivent dans le stockage de WebKit, que la page ne sait pas lire. Sans ca, une mise a jour
// depuis Tauri rendait l'interface en francais par defaut, theme et zoom par defaut. On ne
// passe JAMAIS par-dessus une valeur existante.
try {
  const heritees = ipcRenderer.sendSync('cockpit:preferences-heritees')
  for (const [cle, valeur] of Object.entries(heritees || {})) {
    if (localStorage.getItem(cle) === null) localStorage.setItem(cle, valeur)
  }
} catch (e) {
  console.warn(`preferences de l'ancienne version non reprises : ${e}`)
}
