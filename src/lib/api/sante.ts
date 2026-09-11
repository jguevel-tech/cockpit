import { invoke } from "../coquille";

/// La page a-t-elle peint depuis son dernier passage, la fenetre etait-elle visible et
/// concentree, et l'utilisateur a-t-il touche le clavier ou la souris recemment ?
/// `concentre` disculpe la fenetre recouverte : elle reste « visible » pour la page mais
/// perd le focus, et un gel qu'on ne regarde pas n'est pas une panne. `entreeRecente` dit
/// si quelqu'un est devant : le guetteur ne recharge la vue que dans ce cas.
export const santePage = (
  aPeint: boolean,
  visible: boolean,
  concentre: boolean,
  entreeRecente: boolean,
) => invoke<void>("sante_page", { aPeint, visible, concentre, entreeRecente });

/// Le mode secours du rendu : disponible sous Linux seulement (le chemin DMA-BUF est
/// celui de WebKitGTK), et deja active ou non.
export interface ModeSecoursRendu {
  disponible: boolean;
  actif: boolean;
}

export const lireModeSecoursRendu = () =>
  invoke<ModeSecoursRendu>("mode_secours_rendu");

/// Pose ou retire le mode secours. Prend effet au prochain lancement : la variable de
/// WebKitGTK se lit avant l'initialisation de GTK, donc avant que l'application existe.
export const activerModeSecoursRendu = (activer: boolean) =>
  invoke<void>("activer_mode_secours_rendu", { activer });

/// Relance l'application. Passe par le backend et non par le `relaunch()` du plugin
/// process : le backend libere le nom « single instance » AVANT de lancer la nouvelle
/// instance, sinon celle-ci se tue en le trouvant pris.
export const relancerApplication = () => invoke<void>("relancer_application");
