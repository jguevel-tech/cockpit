/**
 * La disposition des volets d'un onglet Terminaux : un arbre, et rien d'autre.
 *
 * **POURQUOI UN MODULE A PART.** Le composant qui affiche les terminaux fait deja mille
 * lignes et touche au chemin de frappe, qu'on ne bouscule pas. L'algebre de l'arbre — diviser,
 * retirer, redimensionner, relire ce qui a ete range en base — n'a besoin ni du DOM ni d'xterm
 * pour etre juste : elle vit ici, et elle s'eprouve sous node, sans rien installer.
 *
 * **UN VOLET = UNE SESSION EXISTANTE.** L'arbre ne porte que des identifiants : il ne cree
 * jamais de terminal (c'est `addTerminal` qui le fait, parce que lui seul mesure son
 * conteneur) et ne sait pas ce qu'un terminal contient.
 */

/** Une division : deux enfants, un sens, et la part prise par le premier. */
export interface Division {
  type: "division";
  /** `colonnes` : cote a cote. `lignes` : l'un au-dessus de l'autre. */
  sens: "colonnes" | "lignes";
  /** Part du premier enfant, de 0 a 1. Bornee a l'usage pour qu'un volet reste saisissable. */
  ratio: number;
  a: Noeud;
  b: Noeud;
}

/** Un volet : la session qu'il affiche. */
export interface Feuille {
  type: "feuille";
  id: number;
}

export type Noeud = Feuille | Division;

/** Le chemin d'un noeud depuis la racine : la suite des cotes a suivre. */
export type Chemin = ("a" | "b")[];

/** Aucun volet ne descend sous cette part : un volet de trois pixels ne se rattrape pas. */
export const RATIO_MIN = 0.1;
export const RATIO_MAX = 0.9;

export function feuille(id: number): Feuille {
  return { type: "feuille", id };
}

/** Les sessions affichees, de gauche a droite et de haut en bas. */
export function sessionsAffichees(noeud: Noeud | null): number[] {
  if (!noeud) return [];
  if (noeud.type === "feuille") return [noeud.id];
  return [...sessionsAffichees(noeud.a), ...sessionsAffichees(noeud.b)];
}

/** Combien de volets. Un onglet a un seul volet n'affiche aucun separateur. */
export function nombreDeVolets(noeud: Noeud | null): number {
  return sessionsAffichees(noeud).length;
}

/**
 * Divise le volet qui affiche `cible` et met `nouveau` a cote.
 *
 * Rend l'arbre inchange si la cible n'y est pas : diviser un volet ferme entre-temps ne doit
 * pas faire disparaitre la disposition.
 */
export function diviser(
  noeud: Noeud,
  cible: number,
  sens: Division["sens"],
  nouveau: number,
): Noeud {
  if (noeud.type === "feuille") {
    if (noeud.id !== cible) return noeud;
    return { type: "division", sens, ratio: 0.5, a: noeud, b: feuille(nouveau) };
  }
  return {
    ...noeud,
    a: diviser(noeud.a, cible, sens, nouveau),
    b: diviser(noeud.b, cible, sens, nouveau),
  };
}

/**
 * Retire le volet qui affiche `cible`. **Le voisin prend toute la place** : c'est ce a quoi on
 * s'attend en fermant un volet, et ca evite un trou qu'il faudrait ensuite refermer a la main.
 *
 * Rend `null` quand il ne reste plus rien : l'appelant affiche alors son ecran vide.
 */
export function retirer(noeud: Noeud | null, cible: number): Noeud | null {
  if (!noeud) return null;
  if (noeud.type === "feuille") return noeud.id === cible ? null : noeud;
  const a = retirer(noeud.a, cible);
  const b = retirer(noeud.b, cible);
  if (a && b) return { ...noeud, a, b };
  return a ?? b;
}

/** Le ratio d'une division, borne pour qu'aucun des deux volets ne devienne insaisissable. */
export function fixerRatio(noeud: Noeud, chemin: Chemin, ratio: number): Noeud {
  const borne = Math.min(RATIO_MAX, Math.max(RATIO_MIN, ratio));
  if (chemin.length === 0) {
    return noeud.type === "division" ? { ...noeud, ratio: borne } : noeud;
  }
  if (noeud.type !== "division") return noeud;
  const [cote, ...suite] = chemin;
  return cote === "a"
    ? { ...noeud, a: fixerRatio(noeud.a, suite, ratio) }
    : { ...noeud, b: fixerRatio(noeud.b, suite, ratio) };
}

/**
 * Ne garde que les volets dont la session existe encore.
 *
 * **C'EST CE QUI REND LA DISPOSITION SURE A RELIRE.** Elle est rangee en base et les sessions
 * vivent ailleurs : une session fermee sur une autre machine, ou un service repart a neuf,
 * laisserait sinon un volet qui n'affiche rien et ne se ferme pas.
 */
export function nettoyer(noeud: Noeud | null, existantes: Iterable<number>): Noeud | null {
  const vivantes = new Set(existantes);
  const filtrer = (n: Noeud | null): Noeud | null => {
    if (!n) return null;
    if (n.type === "feuille") return vivantes.has(n.id) ? n : null;
    const a = filtrer(n.a);
    const b = filtrer(n.b);
    if (a && b) return { ...n, a, b };
    return a ?? b;
  };
  return filtrer(noeud);
}

/** Le chemin du volet qui affiche `cible`, ou `null`. Sert a viser une division voisine. */
export function cheminDe(noeud: Noeud | null, cible: number, chemin: Chemin = []): Chemin | null {
  if (!noeud) return null;
  if (noeud.type === "feuille") return noeud.id === cible ? chemin : null;
  return (
    cheminDe(noeud.a, cible, [...chemin, "a"]) ?? cheminDe(noeud.b, cible, [...chemin, "b"])
  );
}

/**
 * Affiche la session `id` dans la disposition, sans jamais reduire le nombre de volets.
 *
 * **AFFICHER UN TERMINAL NE FERME PAS LES VOLETS, ET L'OUBLI A ETE LIVRE (0.63.0).** Sur une
 * disposition VIDE, montrer un terminal donne evidemment un volet unique. Le piege est que la
 * disposition rangee en base est relue de facon asynchrone : revenir sur l'onglet Terminal en
 * cliquant un terminal de la barre laterale activait la session AVANT la relecture, donc sur
 * une disposition vide, donc les volets disparaissaient — ils etaient pourtant intacts en base,
 * simplement jamais relus. La regle vit ici, avec ses essais, et non plus dans un composant de
 * mille lignes ou personne ne peut l'eprouver.
 *
 * Quand la session n'est pas affichee, elle PREND LA PLACE du volet actif : « ouvre ce
 * terminal ICI » plutot que « ferme mes volets ».
 */
export function poserLaSession(
  noeud: Noeud | null,
  id: number,
  actif: number | null,
): Noeud {
  if (!noeud) return feuille(id);
  const affichees = sessionsAffichees(noeud);
  if (affichees.includes(id)) return noeud;
  const remplace = actif !== null && affichees.includes(actif) ? actif : affichees[0];
  return remplacerFeuille(noeud, remplace, id);
}

/** Remplace la session d'un volet par une autre, sans toucher a la geometrie. */
export function remplacerFeuille(noeud: Noeud, cible: number, remplacant: number): Noeud {
  if (noeud.type === "feuille") {
    return noeud.id === cible ? feuille(remplacant) : noeud;
  }
  return {
    ...noeud,
    a: remplacerFeuille(noeud.a, cible, remplacant),
    b: remplacerFeuille(noeud.b, cible, remplacant),
  };
}

/** Ou l'on lache un volet sur un autre. `centre` echange les deux. */
export type Cote = "gauche" | "droite" | "haut" | "bas" | "centre";

/**
 * Insere `nouveau` a cote du volet qui affiche `cible`.
 *
 * **`diviser` NE SUFFIT PAS : elle met toujours le nouveau APRES.** Pour lacher un volet en
 * haut ou a gauche, il faut pouvoir le mettre AVANT — sinon deplacer vers la gauche donnerait
 * le meme resultat que vers la droite, et le geste mentirait.
 */
export function inserer(
  noeud: Noeud,
  cible: number,
  sens: Division["sens"],
  nouveau: number,
  avant: boolean,
): Noeud {
  if (noeud.type === "feuille") {
    if (noeud.id !== cible) return noeud;
    const neuf = feuille(nouveau);
    return {
      type: "division",
      sens,
      ratio: 0.5,
      a: avant ? neuf : noeud,
      b: avant ? noeud : neuf,
    };
  }
  return {
    ...noeud,
    a: inserer(noeud.a, cible, sens, nouveau, avant),
    b: inserer(noeud.b, cible, sens, nouveau, avant),
  };
}

/** Echange la place de deux volets, sans toucher a la forme de l'arbre ni aux ratios. */
export function echanger(noeud: Noeud, un: number, autre: number): Noeud {
  if (noeud.type === "feuille") {
    if (noeud.id === un) return feuille(autre);
    if (noeud.id === autre) return feuille(un);
    return noeud;
  }
  return { ...noeud, a: echanger(noeud.a, un, autre), b: echanger(noeud.b, un, autre) };
}

/**
 * Deplace le volet `source` contre le volet `cible`, du cote demande.
 *
 * **LE VOLET EST RETIRE AVANT D'ETRE REPOSE**, sinon il apparaitrait deux fois. Consequence
 * a connaitre : retirer peut faire remonter un voisin et changer la forme de l'arbre autour
 * de la cible — c'est voulu, c'est ce qui evite de laisser un trou la ou le volet etait.
 *
 * Rend l'arbre INCHANGE quand le deplacement n'a pas de sens (sur soi-meme, ou un volet
 * ferme entre-temps) : un geste sans effet vaut mieux qu'une disposition cassee.
 */
export function deplacer(racine: Noeud, source: number, cible: number, cote: Cote): Noeud {
  if (source === cible) return racine;
  const presents = sessionsAffichees(racine);
  if (!presents.includes(source) || !presents.includes(cible)) return racine;
  if (cote === "centre") return echanger(racine, source, cible);
  const sans = retirer(racine, source);
  // Deux volets seulement : en retirer un ne laisse qu'une feuille, qui est la cible.
  if (!sans) return racine;
  const sens = cote === "gauche" || cote === "droite" ? "colonnes" : "lignes";
  return inserer(sans, cible, sens, source, cote === "gauche" || cote === "haut");
}

/**
 * Relit une disposition rangee en base. **Tolerante par construction** : tout ce qui n'est pas
 * exactement la forme attendue rend `null`, et l'onglet repart sur un volet unique. Une
 * disposition illisible ne doit jamais empecher d'ouvrir ses terminaux.
 */
export function depuisJson(texte: string | null | undefined): Noeud | null {
  if (!texte) return null;
  try {
    return valider(JSON.parse(texte));
  } catch {
    return null;
  }
}

function valider(valeur: unknown): Noeud | null {
  if (!valeur || typeof valeur !== "object") return null;
  const brut = valeur as Record<string, unknown>;
  if (brut.type === "feuille") {
    return typeof brut.id === "number" && Number.isFinite(brut.id) ? feuille(brut.id) : null;
  }
  if (brut.type !== "division") return null;
  const a = valider(brut.a);
  const b = valider(brut.b);
  if (!a || !b) return null;
  const sens = brut.sens === "lignes" ? "lignes" : "colonnes";
  const ratio = typeof brut.ratio === "number" && Number.isFinite(brut.ratio) ? brut.ratio : 0.5;
  return {
    type: "division",
    sens,
    ratio: Math.min(RATIO_MAX, Math.max(RATIO_MIN, ratio)),
    a,
    b,
  };
}
