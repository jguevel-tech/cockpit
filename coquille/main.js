// Coquille Electron de Cockpit : une fenetre Chromium qui affiche l'interface existante.
//
// **POURQUOI CE CHANTIER.** Le journal du guetteur compte 1 143 episodes ou la page parle
// encore et ne peint plus aucune image, contre 4 ou notre propre boucle bloquait. Le defaut
// est dans WebKitGTK avec le pilote NVIDIA proprietaire, il date de la 2.42, l'issue Tauri
// qui le recense est ouverte et sans correctif amont. Le seul contournement connu coupe
// DMA-BUF et rend la frappe en retard d'une touche : les deux sorties sont mauvaises.
//
// **CE QUI NE CHANGE PAS.** Le service de terminaux reste le binaire Rust, detache, avec son
// protocole binaire a lui. Il ne sait pas qui l'affiche et n'a pas a le savoir.

const { app, BrowserWindow, protocol, net, shell, ipcMain } = require('electron')
const path = require('node:path')
const { pathToFileURL } = require('node:url')
const { spawn } = require('node:child_process')
const readline = require('node:readline')

// L'interface buildee par Vite. Servie par un protocole a nous plutot qu'en `file://` :
// **une origine stable est ce qui garde le localStorage**, ou vivent la langue et les
// preferences. Un `file://` donne une origine opaque et les perdrait a chaque lancement.
const RACINE_INTERFACE = path.join(__dirname, '..', 'dist')
const SCHEMA = 'cockpit'

protocol.registerSchemesAsPrivileged([
  {
    scheme: SCHEMA,
    privileges: { standard: true, secure: true, supportFetchAPI: true, stream: true }
  }
])

function servirInterface() {
  protocol.handle(SCHEMA, (requete) => {
    const url = new URL(requete.url)
    const relatif = decodeURIComponent(url.pathname)
    // Tout ce qui n'est pas un fichier connu retombe sur index.html : la navigation de
    // l'interface n'a pas de routeur, mais un rechargement profond ne doit pas rendre 404.
    const vise = path.join(RACINE_INTERFACE, relatif === '/' ? 'index.html' : relatif)
    // **JAMAIS SERVIR HORS DE dist/** : un `..` dans le chemin lirait le disque de
    // l'utilisateur depuis la page. Le chemin est resolu puis verifie, pas assaini.
    const resolu = path.resolve(vise)
    if (resolu !== RACINE_INTERFACE && !resolu.startsWith(RACINE_INTERFACE + path.sep)) {
      return new Response('chemin refuse', { status: 403 })
    }
    return net.fetch(pathToFileURL(resolu).toString())
  })
}



// --- Le backend : un processus a part, qui parle en lignes JSON ---------------------------

/**
 * Lance le backend Rust en mode pont et tient la conversation avec lui.
 *
 * **C'EST UN PROCESSUS SEPARE, ET C'EST VOULU.** Le backend ouvre la base, parle au service
 * de terminaux et lit le disque ; le faire vivre dans le processus de la fenetre le
 * rendrait solidaire du moteur de rendu, qui est justement ce qu'on remplace. Separe, il
 * survit a un rechargement de la vue.
 */
class Backend {
  constructor(chemin) {
    this.enAttente = new Map()
    this.prochainAppel = 1
    this.surEvenement = () => {}
    // La sortie d'erreur du backend n'est PAS le protocole : elle va au journal de la
    // coquille, sinon une panne de demarrage serait invisible.
    this.processus = spawn(chemin, ['--pont'], { stdio: ['pipe', 'pipe', 'pipe'] })
    this.processus.stderr.on('data', (bloc) => console.error(`[backend] ${bloc}`.trimEnd()))
    this.processus.on('exit', (code) => {
      // Toute promesse en vol doit etre rejetee : sans ca, l'interface attendrait pour
      // toujours une reponse qui ne viendra jamais, sans rien afficher.
      for (const { rejeter } of this.enAttente.values()) {
        rejeter(new Error(`le backend s'est arrete (code ${code})`))
      }
      this.enAttente.clear()
    })
    readline
      .createInterface({ input: this.processus.stdout })
      .on('line', (ligne) => this.recevoir(ligne))
  }

  recevoir(ligne) {
    let message
    try {
      message = JSON.parse(ligne)
    } catch {
      // Une ligne illisible ne tue pas le pont cote backend ; elle ne doit pas le tuer ici
      // non plus. On la signale et on continue de lire.
      console.error(`[backend] ligne illisible : ${ligne.slice(0, 200)}`)
      return
    }
    if (message.evenement !== undefined) {
      this.surEvenement(message.evenement, message.charge)
      return
    }
    const attente = this.enAttente.get(message.id)
    if (!attente) return
    this.enAttente.delete(message.id)
    if ('err' in message) attente.rejeter(new Error(message.err))
    else attente.resoudre(message.ok)
  }

  appeler(commande, arguments_) {
    const id = this.prochainAppel++
    return new Promise((resoudre, rejeter) => {
      this.enAttente.set(id, { resoudre, rejeter })
      this.processus.stdin.write(`${JSON.stringify({ id, commande, arguments: arguments_ })}\n`)
    })
  }

  arreter() {
    // Fermer l'entree suffit : le pont s'arrete quand son entree se ferme, ce qui le laisse
    // finir proprement au lieu de le tuer au milieu d'une ecriture en base.
    this.processus.stdin.end()
  }
}

/** Le binaire du backend. En developpement, celui que `cargo build` vient de produire. */
function cheminDuBackend() {
  return (
    process.env.COCKPIT_BACKEND ||
    path.join(__dirname, '..', 'src-tauri', 'target', 'debug', 'cockpit')
  )
}

// --- Le pont : ce que la page peut demander --------------------------------------------

/**
 * Les abonnements de la page, par nom d'evenement. Tauri tient cette table cote Rust et
 * reveille la page en EVALUANT du JavaScript ; ici elle vit dans le processus principal et
 * la page n'est rappelee que par identifiant. Rien n'est evalue nulle part.
 */
const abonnements = new Map()
let prochainAbonnement = 1

/** Pousse un evenement vers la page. Le pont vers le backend Rust appellera ceci. */
function pousserEvenement(nom, charge, fenetre) {
  const parNom = abonnements.get(nom)
  if (!parNom) return
  for (const [identifiantAbonnement, rappel] of parNom) {
    // La forme est celle que `listen` attend : sans `payload`, chaque ecouteur de
    // l'interface lirait `undefined` sans qu'une seule erreur ne soit levee.
    fenetre.webContents.send('cockpit:rappel', rappel, {
      event: nom,
      id: identifiantAbonnement,
      payload: charge
    })
  }
}

/**
 * Les commandes tenues par la coquille elle-meme. Le reste part au backend Rust.
 *
 * **UNE COMMANDE INCONNUE EST REFUSEE ET NOMMEE.** Rendre `undefined` donnerait une
 * interface qui s'affiche et ment : des listes vides, des reglages muets, et rien dans le
 * journal. Le refus dit laquelle manque, ce qui donne aussi l'ordre de portage.
 */
function traiterDansLaCoquille(commande, arguments_, fenetre) {
  switch (commande) {
    case 'plugin:event|listen': {
      const { event: nom, handler } = arguments_
      const identifiantAbonnement = prochainAbonnement++
      if (!abonnements.has(nom)) abonnements.set(nom, new Map())
      abonnements.get(nom).set(identifiantAbonnement, handler)
      return { traite: true, valeur: identifiantAbonnement }
    }
    case 'plugin:event|unlisten': {
      const { event: nom, eventId } = arguments_
      abonnements.get(nom)?.delete(eventId)
      return { traite: true, valeur: null }
    }
    case 'plugin:event|emit':
    case 'plugin:event|emit_to': {
      pousserEvenement(arguments_.event, arguments_.payload, fenetre)
      return { traite: true, valeur: null }
    }
    case 'set_webview_zoom':
      // Le zoom appartient a l'HOTE, pas au backend : sous Tauri la commande recevait la
      // fenetre, ici c'est Chromium qui l'applique. Le backend n'a jamais eu a le savoir.
      fenetre.webContents.setZoomFactor(arguments_.factor)
      return { traite: true, valeur: null }
    case 'plugin:app|version':
      return { traite: true, valeur: require('../package.json').version }
    case 'plugin:app|name':
      return { traite: true, valeur: 'Cockpit' }
    default:
      return { traite: false }
  }
}

function brancherLePont(fenetre) {
  const backend = new Backend(cheminDuBackend())
  // Ce que le backend pousse de lui-meme (sortie de terminal, fin de processus) emprunte
  // le meme chemin qu'un evenement emis dans la coquille : l'interface ne voit pas la
  // difference, et n'a pas a la voir.
  backend.surEvenement = (nom, charge) => pousserEvenement(nom, charge, fenetre)
  fenetre.on('closed', () => backend.arreter())

  ipcMain.handle('cockpit:commande', async (_evenement, commande, arguments_) => {
    const dansLaCoquille = traiterDansLaCoquille(commande, arguments_ ?? {}, fenetre)
    if (dansLaCoquille.traite) return dansLaCoquille.valeur
    return backend.appeler(commande, arguments_ ?? {})
  })
}

function ouvrirLaFenetre() {
  const fenetre = new BrowserWindow({
    width: 1400,
    height: 900,
    show: false,
    backgroundColor: '#1a1a1a',
    webPreferences: {
      preload: path.join(__dirname, 'preload.js'),
      // Les trois lignes qui comptent : la page ne voit ni Node, ni le processus
      // principal, et tourne dans le bac a sable de Chromium.
      contextIsolation: true,
      nodeIntegration: false,
      sandbox: true
    }
  })

  // Rien ne navigue hors de notre schema, et un lien externe part dans le NAVIGATEUR de
  // l'utilisateur, jamais dans une fenetre a nous : une page distante dans la coquille
  // aurait acces au pont.
  fenetre.webContents.on('will-navigate', (evenement, url) => {
    if (new URL(url).protocol !== `${SCHEMA}:`) {
      evenement.preventDefault()
      void shell.openExternal(url)
    }
  })
  fenetre.webContents.setWindowOpenHandler(({ url }) => {
    void shell.openExternal(url)
    return { action: 'deny' }
  })

  // Affichee seulement quand elle a quelque chose a montrer : sinon on voit un cadre vide.
  fenetre.once('ready-to-show', () => fenetre.show())
  void fenetre.loadURL(`${SCHEMA}://interface/`)
  brancherLePont(fenetre)
  if (process.env.COCKPIT_BANC_CAPTURE) armerLeBanc(fenetre)
  return fenetre
}

/**
 * Mode banc : capture la page puis quitte, en rapportant ce que la page a signale.
 *
 * **La capture vient de la PAGE, pas de l'ecran.** Un `xwd` sur l'affichage rend le bureau
 * virtuel et depend d'un serveur X ; `capturePage` rend ce que le moteur a peint, donc
 * exactement ce qu'on veut prouver. C'est aussi ce qui permet de mesurer sans jamais
 * ouvrir de fenetre sur l'ecran de quelqu'un.
 */
function armerLeBanc(fenetre) {
  const fs = require('node:fs')
  const destination = process.env.COCKPIT_BANC_CAPTURE
  const plaintes = []
  // Les erreurs de la page ne remontent pas dans stdout du processus principal : sans cet
  // ecouteur, une interface entierement cassee rendrait une capture noire et un journal vide.
  fenetre.webContents.on('console-message', (_evenement, niveau, message, ligne, source) => {
    if (niveau >= 2) plaintes.push(`${source}:${ligne} ${message}`)
  })
  fenetre.webContents.on('render-process-gone', (_e, details) =>
    plaintes.push(`LE RENDU EST MORT : ${details.reason}`)
  )
  fenetre.webContents.once('did-finish-load', () => {
    // Laisser l'interface se monter : ce qui casse au montage doit avoir eu le temps de crier.
    setTimeout(async () => {
      try {
        const image = await fenetre.webContents.capturePage()
        fs.writeFileSync(destination, image.toPNG())
        fs.writeFileSync(`${destination}.plaintes.txt`, plaintes.join('\n') || '(aucune)')
      } catch (e) {
        fs.writeFileSync(`${destination}.plaintes.txt`, `capture impossible : ${e}`)
      }
      app.exit(0)
    }, 4000)
  })
}

app.whenReady().then(() => {
  servirInterface()
  ouvrirLaFenetre()
  app.on('activate', () => {
    if (BrowserWindow.getAllWindows().length === 0) ouvrirLaFenetre()
  })
})

app.on('window-all-closed', () => {
  if (process.platform !== 'darwin') app.quit()
})
