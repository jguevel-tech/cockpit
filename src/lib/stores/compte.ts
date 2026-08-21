/**
 * Le compte : connexion, inscription, appairage par le navigateur.
 *
 * Tout est facultatif. Rien ici ne doit devenir un passage oblige, et une panne reseau ne doit
 * degrader aucune autre fonctionnalite : c'est la promesse faite a l'utilisateur, et c'est
 * aussi ce qui rend le logiciel utilisable dans un train.
 */
import { writable, get } from "svelte/store";
import * as api from "../api/compte";
import type { EtatCompte, DemandeAppairage } from "../api/compte";
import { openUrl } from "../api/workspace";
import { signalerErreur } from "./errors";

export const compte = writable<EtatCompte | null>(null);
/** Passe a vrai quand on a interroge le backend UNE fois, quel qu'en soit le resultat. */
export const compteConnu = writable(false);

/**
 * Un motif de refus rendu par le serveur, sous forme de CLE.
 *
 * L'affichage traduit ; ce magasin ne fabrique jamais de phrase. Une cle inconnue existe : le
 * serveur peut en ajouter avant que le logiciel les connaisse, l'affichage a donc un repli.
 */
export const dernierRefus = writable<string | null>(null);

export async function chargerCompte(): Promise<void> {
  try {
    compte.set(await api.etatCompte());
  } catch (e) {
    signalerErreur("compte.charger", String(e));
  } finally {
    compteConnu.set(true);
  }
}

async function tenter(action: () => Promise<EtatCompte>): Promise<boolean> {
  dernierRefus.set(null);
  try {
    compte.set(await action());
    return true;
  } catch (e) {
    // Un refus attendu (adresse deja prise, mot de passe faux) est une CLE, pas une panne :
    // il s'affiche dans le formulaire. Le reste part dans la remontee d'erreurs.
    const motif = String(e);
    dernierRefus.set(motif);
    if (motif === "serveur") signalerErreur("compte.action", motif);
    return false;
  }
}

export const sInscrire = (email: string, motDePasse: string) =>
  tenter(() => api.inscription(email, motDePasse));

export const seConnecter = (email: string, motDePasse: string) =>
  tenter(() => api.connexion(email, motDePasse));

export const seDeconnecter = () => tenter(() => api.deconnexion());

export const definirServeur = (url: string) => tenter(() => api.definirServeur(url));

/** Intervalle de scrutation : assez court pour que ce soit immediat a l'oeil, assez espace
 *  pour ne pas marteler le serveur pendant qu'on remplit un formulaire dans le navigateur. */
const PAS_DE_SCRUTATION = 2000;

/**
 * Ouvre le navigateur et attend que la connexion y aboutisse.
 *
 * Rend la demande en cours pour que l'ecran puisse afficher le code — il doit correspondre a
 * celui de la page, sinon la demande vient d'ailleurs.
 *
 * `arreter()` doit etre appele si l'utilisateur abandonne : sans ca, la scrutation continue
 * apres la fermeture de l'ecran.
 */
export async function appairerParLeNavigateur(): Promise<{
  demande: DemandeAppairage;
  fini: Promise<boolean>;
  arreter: () => void;
} | null> {
  dernierRefus.set(null);

  let demande: DemandeAppairage;
  try {
    demande = await api.demarrerAppairage();
  } catch (e) {
    dernierRefus.set(String(e));
    return null;
  }

  try {
    await openUrl(demande.url);
  } catch (e) {
    // Le navigateur n'a pas pu s'ouvrir : la demande reste valable, l'ecran affiche l'adresse
    // pour qu'elle soit ouverte a la main. Ne PAS abandonner silencieusement.
    signalerErreur("compte.appairage.navigateur", String(e));
  }

  let vivant = true;
  const arreter = () => {
    vivant = false;
  };

  const fini = (async () => {
    while (vivant) {
      await new Promise((r) => setTimeout(r, PAS_DE_SCRUTATION));
      if (!vivant) return false;
      try {
        const etat = await api.etatAppairage(demande.id);
        if (etat.etat === "accorde") {
          compte.set(etat.compte);
          return true;
        }
      } catch (e) {
        // Expiration ou panne : dans les deux cas la scrutation s'arrete, et l'ecran dit
        // pourquoi. Continuer a interroger une demande morte n'apporterait rien.
        dernierRefus.set(String(e));
        return false;
      }
    }
    return false;
  })();

  return { demande, fini, arreter };
}

export function estConnecte(): boolean {
  return get(compte)?.connecte === true;
}
