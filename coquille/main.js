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

const { app, BrowserWindow, Menu, protocol, net, shell, ipcMain, dialog, session } = require('electron')
const fs = require('node:fs')
const profils = require('./profils')
const path = require('node:path')
const { pathToFileURL } = require('node:url')
const { spawn } = require('node:child_process')
const { traiterUneCommandeDeMiseAJour } = require('./updater')
const { journaliser, dossierDeDonnees } = require('./journal')
const readline = require('node:readline')

// **LE NOM DE L'APPLICATION EST CE QUE LA FENETRE ANNONCE AU BUREAU, ET LES RACCOURCIS
// INSTALLES ATTENDENT « cockpit ».** Sans cette ligne, Electron prend le nom du paquet npm
// (`cockpit-coquille`) : le bureau n'associe plus la fenetre au lanceur, l'icone devient
// generique et une SECONDE entree apparait dans la barre des taches a cote de la vraie.
// Renommer l'executable ne suffit pas — mesure le 2026-09-10 sur le paquet 0.59.2, dont le
// binaire s'appelle bien `cockpit` et dont la fenetre annoncait toujours `cockpit-coquille`.
// Pose AVANT que la fenetre existe, sinon elle garde l'ancien nom. Le dossier de donnees ne
// bouge pas : il est fixe explicitement plus bas.
app.setName('cockpit')

// **UNE PANNE DU PROCESSUS PRINCIPAL DOIT LAISSER UNE TRACE, ET LA BOITE PAR DEFAUT N'EN
// LAISSE AUCUNE.** Le 2026-09-11, apres une mise a jour, une fenetre du systeme s'est
// affichee avant l'interface chez l'utilisateur puis a disparu : introuvable ensuite dans
// le journal de l'application, dans celui de la session et dans Crashpad. Sans ces deux
// ecouteurs, Electron affiche « A JavaScript error occurred in the main process », un texte
// que personne ne peut rapporter et qui n'est ecrit nulle part.
//
// On NE QUITTE PAS : l'application survit a la plupart de ces erreurs (c'est ce que
// l'utilisateur a constate), et se fermer la rendrait plus grave qu'elle n'est. On ne
// montre la fenetre QU'UNE FOIS par lancement — une erreur qui se repete afficherait sinon
// une boite a chaque tour, et c'est exactement le genre de boucle qui prend le poste en
// otage.
let panneDejaMontree = false
function signalerUnePanne(portee, erreur) {
  const texte = erreur && erreur.stack ? erreur.stack : String(erreur)
  journaliser(portee, texte)
  if (panneDejaMontree) return
  panneDejaMontree = true
  // **LA LANGUE CHOISIE N'EST PAS LISIBLE ICI.** Elle vit dans le `localStorage` de la page,
  // et cette boite s'affiche justement quand la page n'a pas demarre. On prend donc celle du
  // systeme, et le francais par defaut, comme partout dans le projet. Deux libelles courts
  // ecrits sur place : les catalogues de `src/` sont chargés par l'interface, pas par nous.
  const enAnglais = !`${app.getLocale() || 'fr'}`.toLowerCase().startsWith('fr')
  const journal = path.join(dossierDeDonnees(), 'logs', 'cockpit.log')
  const titre = enAnglais ? 'Cockpit ran into an error' : 'Cockpit a rencontre une erreur'
  const corps = enAnglais
    ? `${texte}\n\nCockpit keeps running. The details are saved here:\n${journal}`
    : `${texte}\n\nCockpit continue de fonctionner. Le detail est enregistre ici :\n${journal}`
  // `showErrorBox` marche avant que l'application soit prete, contrairement a `showMessageBox`.
  try {
    dialog.showErrorBox(titre, corps)
  } catch {
    // Pas d'affichage possible (tres tot au demarrage, ou sans serveur graphique) : la
    // ligne de journal ecrite juste au-dessus reste, et c'est elle qui compte.
  }
}
process.on('uncaughtException', (e) => signalerUnePanne('coquille.exception', e))
process.on('unhandledRejection', (e) => signalerUnePanne('coquille.promesse', e))

// L'interface buildee par Vite. Servie par un protocole a nous plutot qu'en `file://` :
// **une origine stable est ce qui garde le localStorage**, ou vivent la langue et les
// preferences. Un `file://` donne une origine opaque et les perdrait a chaque lancement.
// **LES CHEMINS NE SONT PAS LES MEMES UNE FOIS EMPAQUETE.** En developpement l'interface
// et le backend sont a leur place dans le depot ; dans le paquet ils sont des ressources.
// Se tromper ici donne une fenetre blanche sans une ligne d'erreur.
const RACINE_INTERFACE = app.isPackaged
  ? path.join(process.resourcesPath, 'interface')
  : path.join(__dirname, '..', 'dist')
const SCHEMA = 'cockpit'

protocol.registerSchemesAsPrivileged([
  {
    scheme: SCHEMA,
    privileges: { standard: true, secure: true, supportFetchAPI: true, stream: true }
  }
])

/**
 * Les sessions deja servies. **`protocol.handle` NE VAUT QUE POUR LA SESSION OU ON
 * L'APPELLE** : la page d'un profil nomme, dans sa propre partition, resterait blanche.
 */
const sessionsServies = new WeakSet()
function servirLaSession(cible) {
  if (sessionsServies.has(cible)) return
  sessionsServies.add(cible)
  servirInterface(cible)
}

function servirInterface(cible) {
  cible.protocol.handle(SCHEMA, async (requete) => {
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
    const reponse = await net.fetch(pathToFileURL(resolu).toString())
    // **LA CSP EST POSEE SUR LA REPONSE, PAS DANS LE HTML.** Une balise `meta` dans
    // `index.html` serait perdue au prochain build de Vite, qui reecrit ce fichier.
    //
    // Ce que chaque directive paie : `style-src 'unsafe-inline'` parce que Svelte et xterm
    // posent des styles a la volee ; `img-src data: blob:` parce que le fond d'ecran et les
    // avatars sont des data URL ; `connect-src` pour les APIs des fournisseurs d'IA et la
    // synchronisation. Il n'y a PAS de `script-src 'unsafe-eval'` : le code de la page est
    // compile, et le pont ne fait rien evaluer. Pas davantage de `'wasm-unsafe-eval'` : la
    // coloration (Shiki) tourne sur son moteur JavaScript, voir `src/lib/shiki.ts`.
    const entetes = new Headers(reponse.headers)
    entetes.set(
      'Content-Security-Policy',
      [
        "default-src 'self'",
        "script-src 'self'",
        "style-src 'self' 'unsafe-inline'",
        "img-src 'self' data: blob:",
        "font-src 'self' data:",
        "connect-src 'self' https:",
        "object-src 'none'",
        "base-uri 'none'",
        "frame-ancestors 'none'"
      ].join('; ')
    )
    return new Response(reponse.body, { status: reponse.status, headers: entetes })
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
  constructor(chemin, nom) {
    this.enAttente = new Map()
    this.prochainAppel = 1
    this.surEvenement = () => {}
    // Garde pour nommer CE backend dans le journal et dans le message de panne : sans lui,
    // plusieurs fenetres ecrivent au meme endroit et on ne sait plus laquelle a crie.
    this.nom = nom
    // La sortie d'erreur du backend n'est PAS le protocole : elle va au journal de la
    // coquille, sinon une panne de demarrage serait invisible.
    this.vivant = true
    this.dernieresPlaintes = []
    this.processus = spawn(chemin, ['--pont'], {
      stdio: ['pipe', 'pipe', 'pipe'],
      env: profils.environnementDuBackend(process.env, nom)
    })
    this.processus.stderr.on('data', (bloc) => {
      const texte = `${bloc}`.trimEnd()
      journaliser(`backend.stderr[${this.nom ?? 'defaut'}]`, texte)
      // Gardees pour les JOINDRE au rejet : sans elles, une panne de demarrage du backend
      // arrive dans l'interface comme une erreur de flux, qui ne nomme rien.
      this.dernieresPlaintes.push(texte)
      if (this.dernieresPlaintes.length > 5) this.dernieresPlaintes.shift()
    })
    // **UN `spawn` QUI ECHOUE N'EMET PAS `exit`, IL EMET `error`.** Sans cet ecouteur, un
    // backend introuvable ou non executable laissait un flux ferme, et le premier appel
    // ressortait en `ERR_STREAM_WRITE_AFTER_END` au milieu du processus principal : une
    // exception qui ne dit ni quel binaire, ni pourquoi.
    this.processus.on('error', (e) => {
      this.vivant = false
      this.echouer(
        new Error(`backend [${this.nom ?? 'defaut'}] introuvable ou illisible (${chemin}) : ${e.message}`)
      )
    })
    this.processus.on('exit', (code) => {
      this.vivant = false
      // Toute promesse en vol doit etre rejetee : sans ca, l'interface attendrait pour
      // toujours une reponse qui ne viendra jamais, sans rien afficher.
      const plaintes = this.dernieresPlaintes.join(' | ')
      this.echouer(
        new Error(
          `le backend [${this.nom ?? 'defaut'}] s'est arrete (code ${code})${plaintes ? ` : ${plaintes}` : ''}`
        )
      )
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
      journaliser('backend.pont', `ligne illisible : ${ligne.slice(0, 200)}`)
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

  /** Rejette tout ce qui attend, et retient la cause pour les appels suivants. */
  echouer(panne) {
    this.panne = panne
    for (const { rejeter } of this.enAttente.values()) rejeter(panne)
    this.enAttente.clear()
  }

  appeler(commande, arguments_) {
    // **ON N'ECRIT JAMAIS SUR UN FLUX FERME.** Ecrire quand meme levait une exception non
    // rattrapee dans le processus principal, qu'Electron affiche dans une fenetre
    // technique illisible. Ici l'appel est rejete avec la vraie cause, que l'interface
    // remonte comme n'importe quelle erreur de commande.
    if (!this.vivant || this.processus.stdin.destroyed) {
      return Promise.reject(this.panne ?? new Error('le backend ne tourne plus'))
    }
    const id = this.prochainAppel++
    return new Promise((resoudre, rejeter) => {
      this.enAttente.set(id, { resoudre, rejeter })
      this.processus.stdin.write(
        `${JSON.stringify({ id, commande, arguments: arguments_ })}\n`,
        (e) => {
          if (!e) return
          this.enAttente.delete(id)
          rejeter(new Error(`envoi au backend impossible : ${e.message}`))
        }
      )
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
  if (process.env.COCKPIT_BACKEND) return process.env.COCKPIT_BACKEND
  // Le nom du binaire differe sous Windows. Sans le `.exe`, le paquet s'ouvre et ne sert
  // rien : la fenetre s'affiche, chaque commande echoue.
  const nom = process.platform === 'win32' ? 'cockpit.exe' : 'cockpit'
  return app.isPackaged
    ? path.join(process.resourcesPath, nom)
    : path.join(__dirname, '..', 'backend', 'target', 'debug', nom)
}

// --- Le pont : ce que la page peut demander --------------------------------------------

/**
 * Les fenetres ouvertes, par profil (`null` = defaut). **UNE AU PLUS PAR PROFIL** : deux
 * backends sur la meme base et le meme service se marcheraient dessus.
 *
 * Chaque entree : `{ nom, fenetre, backend, ecoutes }`. `ecoutes` est PROPRE a la fenetre —
 * un ensemble commun enverrait a chaque page ce qu'une autre ecoute.
 */
const fenetres = new Map()
/** La meme entree, retrouvee depuis la page qui appelle : `webContents.id → entree`. */
const parPage = new Map()
/** Ce que ramene une relance depuis le bureau. */
let derniereActive = null

ipcMain.on('cockpit:ecouter', (evenement, nom) => parPage.get(evenement.sender.id)?.ecoutes.add(nom))
ipcMain.on('cockpit:ignorer', (evenement, nom) => parPage.get(evenement.sender.id)?.ecoutes.delete(nom))

/**
 * Pousse un evenement vers la page. Le pont vers le backend Rust appellera ceci.
 *
 * **LA FENETRE MEURT AVANT LE BACKEND, ET CA A EXPLOSE CHEZ L'UTILISATEUR.** Vu le
 * 2026-09-11 au redemarrage qui suit une mise a jour : la fenetre est detruite, le backend
 * n'a pas encore fini de vider ce qu'il avait a dire, et la premiere ligne qui arrive
 * appelle `send` sur un objet mort — `TypeError: Object has been destroyed`, une fenetre
 * d'erreur avant l'interface. Le backend est un PROCESSUS SEPARE : il n'y a aucun instant ou
 * l'on puisse garantir qu'il s'est taire avant elle. On verifie donc, a chaque envoi.
 */
function pousserEvenement(nom, charge, entree) {
  if (!entree.ecoutes.has(nom)) return
  const { fenetre } = entree
  if (!fenetre || fenetre.isDestroyed() || fenetre.webContents.isDestroyed()) return
  fenetre.webContents.send('cockpit:evenement', nom, charge)
}

/**
 * Les commandes tenues par la coquille elle-meme. Le reste part au backend Rust.
 *
 * **UNE COMMANDE INCONNUE EST REFUSEE ET NOMMEE.** Rendre `undefined` donnerait une
 * interface qui s'affiche et ment : des listes vides, des reglages muets, et rien dans le
 * journal. Le refus dit laquelle manque, ce qui donne aussi l'ordre de portage.
 */
function traiterDansLaCoquille(commande, arguments_, entree) {
  switch (commande) {
    // **`relancer_application` N'EST PAS TRAITEE ICI, ET C'EST DELIBERE.**
    //
    // Elle l'a ete, avec `app.relaunch()` + `app.quit()`, et le 2026-09-09 ca a rendu une
    // machine inutilisable : le backend ne demarrait pas dans le paquet, l'interface
    // demandait une relance, l'application repartait, echouait encore, redemandait. Des
    // fenetres se sont ouvertes sans fin et il a fallu ETEINDRE le poste.
    //
    // Si on la remet un jour, trois conditions, toutes obligatoires : un COMPTEUR de
    // relances par lancement (une seule, jamais deux), un REFUS de relancer tant que le
    // backend n'a pas repondu au moins une fois, et un delai avant de repartir. Une
    // relance automatique sans borne transforme n'importe quelle panne de demarrage en
    // boucle qui prend le poste en otage.
    //
    // En attendant, la commande est REFUSEE et nommee, comme toute commande inconnue.
    case 'coquille:zoom': {
      // Le zoom appartient a l'HOTE, pas au backend : sous Tauri la commande recevait la
      // fenetre, ici c'est Chromium qui l'applique. Le backend n'a jamais eu a le savoir.
      const { fenetre } = entree
      if (fenetre.isDestroyed() || fenetre.webContents.isDestroyed()) return { traite: true }
      fenetre.webContents.setZoomFactor(arguments_.factor)
      return { traite: true, valeur: null }
    }
    case 'coquille:version':
      // `app.getVersion()` et non un `require` du package.json parent : celui-ci n'est
      // PAS dans le paquet, et l'appel echouait des le demarrage de l'AppImage alors
      // qu'il marchait en developpement.
      return { traite: true, valeur: app.getVersion() }
    case 'coquille:nom':
      return { traite: true, valeur: 'Cockpit' }
    case 'coquille:profils':
      return {
        traite: true,
        valeur: {
          courant: entree.nom,
          profils: profils
            .listerProfils(app.getPath('userData'))
            .map((nom) => ({ nom, ouvert: fenetres.has(nom) }))
        }
      }
    case 'coquille:ouvrir-profil': {
      // Un nom invalide leve ici : l'appel est rejete avec la regle, que la page affiche.
      const nom = arguments_.nom ?? null
      if (nom !== null) profils.validerNom(nom)
      ouvrirLaFenetre(nom)
      return { traite: true, valeur: null }
    }
    default:
      return { traite: false }
  }
}


/**
 * Les selecteurs de fichiers, traduits vers ceux du systeme.
 *
 * **CE SONT DES DIALOGUES NATIFS, ET C'EST L'HOTE QUI LES OUVRE.** Sous Tauri, le plugin
 * `dialog` faisait la meme chose depuis le Rust. Le contrat rendu a la page ne change pas :
 * un chemin, un tableau de chemins si plusieurs sont permis, `null` si l'on annule. Le
 * frontend ne voit aucune difference et n'a pas ete touche.
 */
async function ouvrirUnDialogue(commande, options, fenetre) {
  const proprietes = []
  if (options.directory) proprietes.push('openDirectory')
  else proprietes.push('openFile')
  if (options.multiple) proprietes.push('multiSelections')

  const commun = {
    ...(options.title ? { title: options.title } : {}),
    ...(options.defaultPath ? { defaultPath: options.defaultPath } : {}),
    ...(options.filters ? { filters: options.filters } : {})
  }

  if (commande === 'coquille:dialogue-enregistrer') {
    const { canceled, filePath } = await dialog.showSaveDialog(fenetre, commun)
    return canceled ? null : filePath
  }
  const { canceled, filePaths } = await dialog.showOpenDialog(fenetre, {
    ...commun,
    properties: proprietes
  })
  // Annuler rend `null`, jamais un tableau vide : c'est ce que le frontend teste.
  if (canceled || filePaths.length === 0) return null
  return options.multiple ? filePaths : filePaths[0]
}

function brancherLePont(entree) {
  const { fenetre } = entree
  const backend = new Backend(cheminDuBackend(), entree.nom)
  // Ce que le backend pousse de lui-meme (sortie de terminal, fin de processus) emprunte
  // le meme chemin qu'un evenement emis dans la coquille : l'interface ne voit pas la
  // difference, et n'a pas a la voir.
  backend.surEvenement = (nom, charge) => pousserEvenement(nom, charge, entree)
  // **ON TAIT LE BACKEND DES QUE LA FENETRE PART, ET PAS SEULEMENT AU MOMENT D'ENVOYER.**
  // La garde de `pousserEvenement` protege ce chemin-la ; celle-ci protege TOUS ceux qu'on
  // ajouterait ensuite, parce que le backend n'a alors plus personne a qui parler. `close`
  // arrive AVANT `closed`, donc avant que l'objet soit detruit : c'est le bon moment.
  fenetre.on('close', () => {
    backend.surEvenement = () => {}
  })
  fenetre.on('closed', () => backend.arreter())
  return backend
}

/**
 * **UN SEUL `handle` POUR TOUTES LES FENETRES**, route par la page qui appelle : chaque
 * fenetre parle a SON backend.
 */
ipcMain.handle('cockpit:commande', async (evenement, commande, arguments_) => {
  const entree = parPage.get(evenement.sender.id)
  if (!entree) throw new Error(`commande ${commande} venue d'une page inconnue`)
  const { fenetre, backend } = entree
  const dansLaCoquille = traiterDansLaCoquille(commande, arguments_ ?? {}, entree)
  if (dansLaCoquille.traite) return dansLaCoquille.valeur
  if (commande === 'coquille:dialogue-ouvrir' || commande === 'coquille:dialogue-enregistrer') {
    return ouvrirUnDialogue(commande, arguments_?.options ?? {}, fenetre)
  }
  // La mise a jour : les memes commandes que le plugin de Tauri, servies par
  // electron-updater. L'interface ne voit aucune difference et n'a pas ete touchee.
  const miseAJour = await traiterUneCommandeDeMiseAJour(
    commande,
    arguments_ ?? {},
    (avancement) => pousserEvenement('maj:avancement', avancement, entree)
  )
  if (miseAJour.traite) return miseAJour.valeur
  return backend.appeler(commande, arguments_ ?? {})
})

function ramener(fenetre) {
  // Entre `close` et `closed` la fenetre est encore dans la table mais deja detruite :
  // la ramener leverait sur `isMinimized`.
  if (fenetre.isDestroyed()) return
  if (fenetre.isMinimized()) fenetre.restore()
  fenetre.focus()
}

/** Ouvre la fenetre d'un profil, ou ramene celle qui l'a deja. */
function ouvrirLaFenetre(nom = null) {
  const existante = fenetres.get(nom)
  if (existante && !existante.fenetre.isDestroyed()) {
    ramener(existante.fenetre)
    return existante.fenetre
  }
  // Une entree dont la fenetre est deja detruite (entre `close` et `closed`) ne sert plus
  // a rien : la garder ferait ramener un cadre mort au lieu d'en ouvrir un nouveau.
  if (existante) fenetres.delete(nom)
  // Cree ici et non par le backend : un profil tout juste cree doit deja figurer dans la
  // liste que la page redemande.
  fs.mkdirSync(profils.dossierDuProfil(app.getPath('userData'), nom), { recursive: true })
  const partition = profils.partitionDuProfil(nom)
  servirLaSession(partition ? session.fromPartition(partition) : session.defaultSession)

  const fenetre = new BrowserWindow({
    width: 1400,
    height: 900,
    show: false,
    title: profils.titreDuProfil(nom),
    backgroundColor: '#1a1a1a',
    webPreferences: {
      preload: path.join(__dirname, 'preload.js'),
      // Les trois lignes qui comptent : la page ne voit ni Node, ni le processus
      // principal, et tourne dans le bac a sable de Chromium.
      contextIsolation: true,
      nodeIntegration: false,
      sandbox: true,
      ...(partition ? { partition } : {})
    }
  })
  // Le titre dit le profil : celui que pose la page ne doit pas l'effacer.
  fenetre.on('page-title-updated', (evenement) => evenement.preventDefault())

  const entree = { nom, fenetre, backend: null, ecoutes: new Set() }
  const idDeLaPage = fenetre.webContents.id
  fenetres.set(nom, entree)
  parPage.set(idDeLaPage, entree)
  fenetre.on('focus', () => {
    derniereActive = entree
  })
  fenetre.on('closed', () => {
    // Une entree deja remplacee (fenetre detruite reouverte entre-temps pour ce meme
    // profil) ne doit pas etre effacee ici : ce serait celle de la fenetre NEUVE.
    if (fenetres.get(nom) === entree) fenetres.delete(nom)
    parPage.delete(idDeLaPage)
    if (derniereActive === entree) derniereActive = null
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
  // Maximisee d'emblee : 1400x900 n'est que la taille qu'elle reprend quand on la restaure.
  // Pas au banc : ses captures et ses clics a coordonnees fixes exigent la meme taille quel
  // que soit l'ecran. Les bancs interactifs ne posent pas COCKPIT_BANC_CAPTURE, d'ou la
  // seconde variable, posee par chacun d'eux.
  fenetre.once('ready-to-show', () => {
    if (!process.env.COCKPIT_BANC_CAPTURE && !process.env.COCKPIT_TAILLE_FIXE) fenetre.maximize()
    fenetre.show()
  })
  // La mort du moteur de rendu laissait une fenetre blanche sans un mot : c'est l'autre
  // forme que prend « une erreur au lancement » vue de l'utilisateur.
  fenetre.webContents.on('render-process-gone', (_e, details) =>
    journaliser('coquille.rendu', `le rendu s'est arrete : ${details.reason} (${details.exitCode})`)
  )
  entree.backend = brancherLePont(entree)
  fenetre.loadURL(`${SCHEMA}://interface/`)
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

// **LE DOSSIER DE DONNEES EST CELUI DE L'IDENTIFIANT, PAS CELUI D'ELECTRON.** Sans cette
// ligne, Chromium ecrirait dans un dossier a lui et la page repartirait sans son
// `localStorage` : la langue et les preferences d'interface seraient perdues. Le backend,
// lui, calcule le sien depuis le meme identifiant, donc la base et le fond d'ecran sont
// retrouves quoi qu'il arrive.
// **`appData` VAUT `~/.config` SOUS LINUX, PAS `~/.local/share`.** Mesure le 2026-09-10 :
// le stockage de la page atterrissait a cote de celui du backend. La regle recopiee ici est
// celle du backend (`chemins::calculer_le_dossier_de_donnees`), XDG compris, pour que les deux
// designent le MEME dossier.
app.setPath(
  'userData',
  path.join(
    process.env.XDG_DATA_HOME || path.join(app.getPath('home'), '.local', 'share'),
    'com.cockpit.dev'
  )
)

// **UN SEUL PROCESSUS, ET CE N'EST PAS COSMETIQUE.** Deux Cockpit partagent la meme base ET
// le meme service de terminaux. Tauri posait ce verrou ; sans lui ici, lancer cette version
// pendant que l'ancienne tourne fait travailler deux applications sur les memes donnees. Le
// verrou est pris AVANT tout le reste : plus tard, la seconde instance aurait deja ouvert sa
// fenetre et parle au backend. Ce processus unique tient desormais une fenetre par profil.
if (!app.requestSingleInstanceLock()) {
  app.exit(0)
} else {
  // Relancer depuis le bureau doit ramener une fenetre existante, pas ne rien faire.
  app.on('second-instance', () => {
    const cible = derniereActive ?? [...fenetres.values()].at(-1)
    if (cible) ramener(cible.fenetre)
  })
}

app.whenReady().then(() => {
  // Une ligne par lancement, et elle porte ce qu'on redemande a chaque diagnostic : la
  // version, et si le bac a sable de Chromium est actif. **Il ne l'est PAS sous AppImage** :
  // le montage est `nosuid`, donc le bit SUID de `chrome-sandbox` ne peut pas servir et le
  // lanceur ajoute `--no-sandbox` de lui-meme. Mesure le 2026-09-11 ; ce n'est pas un
  // oubli d'empaquetage, c'est la limite du format.
  journaliser(
    'coquille',
    `demarrage ${app.getVersion()} — ${app.isPackaged ? 'paquet' : 'developpement'}, ` +
      `bac a sable ${process.argv.includes('--no-sandbox') ? 'DESACTIVE' : 'actif'}`
  )
  // **PAS DE MENU D'ELECTRON.** Celui d'office (File, Edit, View, Window, Help) prenait une
  // ligne en haut de la fenetre sans rien apporter, et son zoom contournait celui de
  // l'interface : le pourcentage de l'en-tete ne suivait plus. Le zoom au clavier est repris
  // par l'interface (App.svelte). Sous macOS le menu reste, reduit : c'est lui qui porte
  // Cmd+C / Cmd+V dans les champs, et la barre de menus y est hors de la fenetre.
  Menu.setApplicationMenu(
    process.platform === 'darwin'
      ? Menu.buildFromTemplate([{ role: 'appMenu' }, { role: 'editMenu' }, { role: 'windowMenu' }])
      : null
  )
  ouvrirLaFenetre()
  app.on('activate', () => {
    if (fenetres.size === 0) ouvrirLaFenetre()
  })
})

app.on('window-all-closed', () => {
  if (process.platform !== 'darwin') app.quit()
})
