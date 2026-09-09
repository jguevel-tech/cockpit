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

const { app, BrowserWindow, protocol, net, shell } = require('electron')
const path = require('node:path')
const { pathToFileURL } = require('node:url')

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
