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

/** Appelle une commande. Celles de la coquille commencent par `coquille:`, les autres vont au backend. */
export function invoke<T>(commande: string, arguments_?: Record<string, unknown>): Promise<T> {
  return preload().invoke(commande, arguments_ ?? {}) as Promise<T>;
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
  | { event: "Progress"; data: { chunkLength: number } }
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
