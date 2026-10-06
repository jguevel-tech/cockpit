/**
 * Ce que la page peut demander a la coquille Electron.
 *
 * **C'EST LA SEULE PORTE, ET ELLE EST CELLE D'ELECTRON.** La page tourne dans le bac a
 * sable : elle ne parle au processus principal que par ce que `coquille/preload.js` expose.
 * Ce fichier en donne la version typee, et rien d'autre dans `src/` ne touche
 * `window.cockpit` directement — sinon les types et le traitement des erreurs seraient
 * contournes.
 *
 * Jusqu'a la 0.59.4, cette couche imitait l'API interne de Tauri : `__TAURI_INTERNALS__`,
 * des identifiants de rappel, et des commandes nommees `plugin:event|listen`,
 * `plugin:dialog|open`, `plugin:updater|check`. Le moteur etait parti, sa forme restait.
 * Tout est parti avec.
 */

/** Ce que le preload pose sur la page. */
interface Preload {
  invoke(commande: string, arguments_?: Record<string, unknown>): Promise<unknown>;
  surEvenement(nom: string, fonction: (charge: unknown) => void): Detacher;
  convertirChemin(chemin: string): string;
  cheminDuFichier(fichier: File): string;
}

function preload(): Preload {
  const pont = (globalThis as unknown as { cockpit?: Preload }).cockpit;
  if (!pont) {
    // Un silence serait pire : sans la coquille RIEN ne marche, et il faut que ca se voie.
    throw new Error("la coquille n'est pas branchee : la page est ouverte hors de Cockpit");
  }
  return pont;
}

/**
 * Remet une charge a plat avant de la confier au pont.
 *
 * **LE PONT CLONE CE QU'ON LUI DONNE, ET IL NE SAIT PAS CLONER UN OBJET REACTIF.** Svelte 5
 * represente une valeur `$state` par un Proxy ; le clonage d'Electron le refuse avec « An
 * object could not be cloned » et l'appel ne part JAMAIS. Constate le 2026-09-18 : enregistrer
 * un second namespace echouait des qu'un premier existait, parce que la liste envoyee portait
 * alors un objet venu de l'etat. Le premier marchait, donc le defaut ne se voyait qu'une fois
 * l'ecran deja rempli.
 *
 * **ON NE PAIE QUE CE QU'IL FAUT.** Une charge faite de valeurs simples part telle quelle, sans
 * allocation : c'est le cas du chemin de frappe (`{ id, data }`), ou l'interdit sur les
 * surcouches s'applique. Mesure du 2026-09-18 : remettre a plat une charge de frappe coute
 * 0,63 us, la verification qui l'evite 0,05 us, et l'aller-retour du pont des dizaines de
 * microsecondes.
 *
 * Sans perte : tout ce qui part au backend traverse un tuyau de lignes JSON, donc rien de ce
 * qu'on envoie ne survivrait a un aller-retour JSON de toute facon.
 */
export function aplatir(arguments_: Record<string, unknown>): Record<string, unknown> {
  for (const valeur of Object.values(arguments_)) {
    if (valeur !== null && typeof valeur === "object") {
      return JSON.parse(JSON.stringify(arguments_)) as Record<string, unknown>;
    }
  }
  return arguments_;
}

/// Ce qu'Electron ajoute devant tout refus qui traverse `ipcRenderer.invoke`.
const PREFIXE_ELECTRON = /^(?:Error: )?Error invoking remote method '[^']*': (?:Error: )?/;

/**
 * Le message d'un refus, tel que le backend l'a ecrit.
 *
 * **L'UTILISATEUR LISAIT « Error: Error invoking remote method 'cockpit:commande': Error: »
 * devant chaque message d'erreur**, sur tous les ecrans : Electron enveloppe le refus dans une
 * `Error` et y colle son propre prefixe. Sous Tauri, un refus arrivait en texte nu, et tout
 * l'interface l'affiche par `String(e)` : on rend donc le texte nu, a la seule porte.
 */
export function messageDErreur(e: unknown): string {
  const brut = e instanceof Error ? e.message : String(e);
  return brut.replace(PREFIXE_ELECTRON, "");
}

/** Appelle une commande. Celles de la coquille commencent par `coquille:`, les autres vont au backend. */
export function invoke<T>(commande: string, arguments_?: Record<string, unknown>): Promise<T> {
  return (preload().invoke(commande, aplatir(arguments_ ?? {})) as Promise<T>).catch((e: unknown) => {
    throw messageDErreur(e);
  });
}

/** Ce qu'on appelle pour cesser d'ecouter. */
export type Detacher = () => void;

/** La forme d'un evenement, telle que l'interface la lit. */
export interface Evenement<T> {
  payload: T;
}

/**
 * Ecoute un evenement pousse par le backend ou par la coquille.
 *
 * **S'ABONNER NE COUTE PLUS D'ALLER-RETOUR** : la table des ecouteurs vit dans la page. Le
 * detachement se fait en appelant ce qui est rendu, et il est obligatoire — les composants
 * de terminal survivent aux demontages, donc un abonnement oublie se rejouerait a chaque
 * retour sur l'onglet.
 */
export function ecouter<T>(
  nom: string,
  gestionnaire: (evenement: Evenement<T>) => void,
): Detacher {
  return preload().surEvenement(nom, (charge) => gestionnaire({ payload: charge as T }));
}

/** L'adresse d'une ressource locale, servie par notre schema. */
export function convertirChemin(chemin: string): string {
  return preload().convertirChemin(chemin);
}

/**
 * Le chemin d'un fichier depose dans la fenetre.
 *
 * **`File.path` N'EXISTE PLUS DEPUIS ELECTRON 32** : seul le preload sait repondre.
 */
export function cheminDuFichier(fichier: File): string {
  return preload().cheminDuFichier(fichier);
}

/** La version de l'application. */
export function versionDeLApplication(): Promise<string> {
  return invoke<string>("coquille:version");
}

/** Un profil de fenetre, tel que la coquille le liste. `nom: null` = le profil par defaut. */
export interface ProfilDeFenetre {
  nom: string | null;
  ouvert: boolean;
}

export interface EtatDesProfils {
  /** Le profil de CETTE fenetre. */
  courant: string | null;
  profils: ProfilDeFenetre[];
}

export function profilsDesFenetres(): Promise<EtatDesProfils> {
  return invoke<EtatDesProfils>("coquille:profils");
}

/** Ouvre la fenetre d'un profil (cree s'il est nouveau), ou ramene celle qui l'a deja. */
export function ouvrirLeProfil(nom: string | null): Promise<null> {
  return invoke<null>("coquille:ouvrir-profil", { nom });
}

/** Options communes aux deux dialogues de fichier. */
export interface OptionsDeDialogue {
  title?: string;
  defaultPath?: string;
  directory?: boolean;
  multiple?: boolean;
  filters?: { name: string; extensions: string[] }[];
}

/** Ouvre un fichier ou un dossier. Rend `null` si on annule, jamais un tableau vide. */
export function ouvrirUnDialogue(
  options: OptionsDeDialogue = {},
): Promise<string | string[] | null> {
  return invoke<string | string[] | null>("coquille:dialogue-ouvrir", { options });
}

/** Demande ou enregistrer un fichier. Rend `null` si on annule. */
export function enregistrerParUnDialogue(options: OptionsDeDialogue = {}): Promise<string | null> {
  return invoke<string | null>("coquille:dialogue-enregistrer", { options });
}

/** Une version plus recente, telle que la coquille la decrit. */
export interface MiseAJourTrouvee {
  version: string;
  versionActuelle: string;
  notes: string | null;
  date: string | null;
}

/** Ce que la coquille rapporte pendant le telechargement. */
export type AvancementDeMiseAJour =
  | { event: "Started"; data: { contentLength?: number } }
  | { event: "Progress"; data: { chunkLength: number; transferred?: number; total?: number } }
  | { event: "Finished" };

/** Cherche une version plus recente. Rend `null` quand il n'y a rien de neuf. */
export function chercherUneMiseAJour(): Promise<MiseAJourTrouvee | null> {
  return invoke<MiseAJourTrouvee | null>("coquille:maj-chercher");
}

/**
 * Telecharge la derniere mise a jour trouvee et l'installe.
 *
 * **NE REND PAS LA MAIN** : l'application se ferme pour se remplacer. Rien a relancer
 * ensuite — la coquille s'en charge.
 */
export async function installerLaMiseAJour(
  surAvancement: (avancement: AvancementDeMiseAJour) => void,
): Promise<void> {
  const detacher = ecouter<AvancementDeMiseAJour>("maj:avancement", (e) => surAvancement(e.payload));
  try {
    await invoke<null>("coquille:maj-installer");
  } finally {
    detacher();
  }
}
