// Le pont, et le SEUL point de contact entre la page et le reste du monde.
//
// La page tourne dans le bac a sable : ni Node, ni acces au processus principal. Elle ne
// peut faire que ce qui est expose ici, nommement.
//
// **POURQUOI ON IMITE L'API INTERNE DE TAURI PLUTOT QUE DE REECRIRE L'INTERFACE.** Le
// bundle ne touche `__TAURI_INTERNALS__` que par quatre fonctions et cinq occurrences. Les
// fournir ici laisse les 113 appels de `src/lib/api/` et les 23 fichiers qui ecoutent des
// evenements INCHANGES, octet pour octet. Ce qui ne change pas ne peut pas regresser :
// c'est ce qui rend tenable la promesse d'une migration sans regression.

const { contextBridge, ipcRenderer } = require('electron')

// Les fonctions que la page a confiees au backend, par identifiant. C'est ce que
// `transformCallback` enregistre et ce qu'un evenement pousse vient rappeler. Tauri les
// pose sur `window` et les appelle en EVALUANT du JavaScript ; ici elles restent dans le
// preload et rien n'est evalue, ce qui retire cette surface d'un coup.
const rappels = new Map()
let prochainIdentifiant = 1

// Le backend ne connait que des identifiants : il ne peut donc reveiller qu'une fonction
// que la page a elle-meme enregistree, jamais du code de son choix.
ipcRenderer.on('cockpit:rappel', (_evenement, identifiant, charge) => {
  const rappel = rappels.get(identifiant)
  if (!rappel) return
  if (rappel.uneSeuleFois) rappels.delete(identifiant)
  rappel.fonction(charge)
})

contextBridge.exposeInMainWorld('__TAURI_INTERNALS__', {
  /** Appelle une commande du backend. Meme forme et meme contrat que celle de Tauri. */
  invoke: (commande, arguments_ = {}, options = undefined) =>
    ipcRenderer.invoke('cockpit:commande', commande, arguments_, options),

  /**
   * Confie une fonction au backend et rend son identifiant. Les evenements et les canaux
   * de Tauri passent tous par la : `listen` enregistre son ecouteur ici, puis appelle
   * `invoke('plugin:event|listen', ...)` avec l'identifiant obtenu.
   */
  transformCallback: (fonction, uneSeuleFois = false) => {
    const identifiant = prochainIdentifiant++
    rappels.set(identifiant, { fonction, uneSeuleFois })
    return identifiant
  },

  /** Libere une fonction confiee. Sans ca, un canal ferme fuirait a chaque ouverture. */
  unregisterCallback: (identifiant) => {
    rappels.delete(identifiant)
  },

  /**
   * L'identite de la fenetre et de la vue. `getCurrentWindow()` et `getCurrentWebview()`
   * la lisent au chargement du module, donc AVANT tout appel : elle doit exister des le
   * depart, et pas etre demandee au backend.
   */
  metadata: {
    currentWindow: { label: 'main' },
    currentWebview: { windowLabel: 'main', label: 'main' }
  },

  /** Une ressource locale, servie par notre schema. Aucun fichier hors de dist/ n'est lisible. */
  convertFileSrc: (chemin) => `cockpit://interface/${String(chemin).replace(/^\/+/, '')}`
})

// --- Reprise des preferences de l'ancienne version ---------------------------------------
//
// **CE BLOC S'EXECUTE AVANT LE MOINDRE SCRIPT DE LA PAGE**, et c'est toute la raison de sa
// place ici : la langue, le theme et le zoom sont lus au tout premier rendu. Pose plus tard,
// l'interface serait deja partie en francais par defaut.
//
// L'appel est SYNCHRONE, ce qu'on evite partout ailleurs. Une promesse rendrait la main
// apres que la page a lu ses preferences, donc trop tard. C'est un aller-retour, une fois,
// au demarrage.
try {
  const heritees = ipcRenderer.sendSync('cockpit:preferences-heritees')
  for (const [cle, valeur] of Object.entries(heritees || {})) {
    // **ON N'ECRASE JAMAIS UNE VALEUR EXISTANTE.** Au deuxieme lancement, celle de Chromium
    // est la bonne : reprendre l'ancienne annulerait chaque changement de theme ou de langue.
    if (localStorage.getItem(cle) === null) localStorage.setItem(cle, valeur)
  }
} catch (e) {
  // Une reprise ratee n'empeche pas de demarrer : on repart sur les valeurs par defaut,
  // exactement comme une installation neuve. Mais elle se dit.
  console.warn(`preferences de l'ancienne version non reprises : ${e}`)
}
