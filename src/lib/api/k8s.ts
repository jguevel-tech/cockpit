import { invoke } from "../coquille";
import type { Pod } from "../k8s/vue";

/** Un contexte du kubeconfig, tel qu'il s'affiche dans le selecteur de cluster. */
export interface Contexte {
  nom: string;
  cluster: string;
  serveur: string;
  namespace: string | null;
  courant: boolean;
  /** Rempli quand Cockpit ne sait pas parler a ce cluster : la raison, affichee telle quelle. */
  obstacle: string | null;
}

export interface Vue {
  pods: Pod[];
  sans_mesures: string | null;
  version: string;
}

export const k8sContextes = () => invoke<Contexte[]>("k8s_contextes");
export const k8sNamespaces = (contexte: string) => invoke<string[]>("k8s_namespaces", { contexte });
export const k8sPods = (contexte: string, namespace: string) =>
  invoke<Vue>("k8s_pods", { contexte, namespace });
export const k8sLogs = (
  contexte: string,
  namespace: string,
  pod: string,
  conteneur: string | null,
  lignes: number,
  precedent: boolean,
) => invoke<string>("k8s_logs", { contexte, namespace, pod, conteneur, lignes, precedent });
export const k8sEvenements = (contexte: string, namespace: string, pod: string) =>
  invoke<Record<string, unknown>[]>("k8s_evenements", { contexte, namespace, pod });
export const k8sYaml = (contexte: string, namespace: string, pod: string) =>
  invoke<string>("k8s_yaml", { contexte, namespace, pod });
/** Ce que la fusion d'un kubeconfig a produit. */
export interface Ajout {
  ajoutes: string[];
  deja_la: string[];
  renouveles: string[];
  renommes: string[];
}

/**
 * Ajoute un cluster a partir du kubeconfig telecharge depuis son interface web.
 *
 * Le contenu part au backend et n'en revient jamais : il porte un jeton d'acces.
 */
export const k8sAjouterUnCluster = (kubeconfig: string) =>
  invoke<Ajout>("k8s_ajouter_un_cluster", { kubeconfig });

/** `kubectl` est-il installe ? Seul le bouton « ouvrir un shell » en depend. */
export const k8sKubectlPresent = () => invoke<boolean>("k8s_kubectl_present");

/** Un namespace suivi en continu, avec son rythme. */
export interface Cible {
  contexte: string;
  namespace: string;
  /** Secondes entre deux mesures. */
  periode: number;
  actif: boolean;
}

export interface ReglagesSurveillance {
  cibles: Cible[];
  retention_heures: number;
}

/** Un point d'historique, tel qu'il est range en base. */
export interface PointEnregistre {
  pod: string;
  t: number;
  cpu: number;
  ram: number;
}

export const k8sSurveillanceLire = () => invoke<ReglagesSurveillance>("k8s_surveillance_lire");
/** Rend les reglages TELS QU'ILS ONT ETE ENREGISTRES : le backend borne le rythme. */
export const k8sSurveillanceEcrire = (reglages: ReglagesSurveillance) =>
  invoke<ReglagesSurveillance>("k8s_surveillance_ecrire", { reglages });
export const k8sHistorique = (contexte: string, namespace: string, depuis: number) =>
  invoke<PointEnregistre[]>("k8s_historique", { contexte, namespace, depuis });

/**
 * Ce qui est DECLARE dans le namespace, meme quand rien ne tourne.
 *
 * Une liste de pods ne dit pas ce que le namespace contient : un travail planifie qui ne s'est
 * jamais declenche n'y figure pas, et 79 travaux declares n'y apparaissent que par les 264 pods
 * qu'ils ont laisses. C'est une lecture par ouverture d'ecran, pas une lecture sur minuteur.
 */
export const k8sWorkloads = (contexte: string, namespace: string) =>
  invoke<import("../k8s/vue").Workload[]>("k8s_workloads", { contexte, namespace });

/**
 * Supprime un pod.
 *
 * **LE CLIENT ENVOIE DES NOMS, ET LE CLUSTER TRANCHE.** Comme partout ici, l'interface dit
 * « ce contexte, ce namespace, ce pod » : elle ne connait ni adresse ni jeton, et le backend
 * verifie chaque nom avant de le mettre dans une URL. Un refus du cluster (403) remonte tel
 * quel, parce qu'on ne sait pas a l'avance ce que les droits permettent.
 */
export const k8sSupprimerUnPod = (contexte: string, namespace: string, pod: string) =>
  invoke<void>("k8s_supprimer_un_pod", { contexte, namespace, pod });

/// Lance une tache planifiee maintenant ; rend le nom du travail cree. Memes regles : des
/// noms, verifies par le backend, et le cluster qui tranche les droits.
export const k8sLancerUneTache = (contexte: string, namespace: string, tache: string) =>
  invoke<string>("k8s_lancer_une_tache", { contexte, namespace, tache });
