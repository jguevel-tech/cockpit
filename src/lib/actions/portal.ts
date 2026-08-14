/**
 * Téléporte l'élément dans <body> au montage.
 *
 * OBLIGATOIRE pour tout overlay en `position: fixed` (modal, menu contextuel, panneau).
 * Les conteneurs structurels portent `isolation: isolate` en mode image de fond
 * (components.css) : chacun devient un contexte d'empilement, et un overlay resté
 * enfant d'un de ces conteneurs est peint SOUS les conteneurs suivants dans le DOM —
 * quel que soit son z-index. Constaté le 2026-08-14 : le modal de création de projet,
 * enfant de la sidebar, était invisible (voile gris sur la seule sidebar, dialogue
 * caché derrière le panneau principal) dès qu'un wallpaper était actif.
 * Dans <body>, l'overlay échappe à tous ces contextes.
 */
export function portal(node: HTMLElement) {
  document.body.appendChild(node);
  return {
    destroy() {
      node.remove();
    },
  };
}
