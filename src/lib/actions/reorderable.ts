import type { DropPosition } from "../utils/reorder";

export interface ReorderableParams {
  /** Index de l'element dans sa liste. */
  index: number;
  /** Groupe : seuls les items du meme groupe peuvent etre reordonnes entre eux. */
  group: string;
  /** Appele sur drop : deplacer `from` avant/apres `to`. */
  onDrop: (from: number, to: number, pos: DropPosition) => void;
  /** Desactive le drag (ex: pendant un rename inline). */
  disabled?: boolean;
}

// Etat du drag en cours, partage au niveau module (dataTransfer n'est pas
// lisible pendant dragover). Un seul drag a la fois par definition.
let dragging: { group: string; index: number } | null = null;

/**
 * Action Svelte factorisant le drag & drop de reordonnancement copie-colle
 * dans Sidebar/NoteTree/TodoList/Dashboard. Pose les classes globales
 * `dragging`, `drag-over-top`, `drag-over-bottom` (stylees dans components.css).
 *
 *   <li use:reorderable={{ index: i, group: "todos", onDrop: moveTodo }}>
 */
export function reorderable(node: HTMLElement, params: ReorderableParams) {
  let p = params;

  function clearOver() {
    node.classList.remove("drag-over-top", "drag-over-bottom");
  }

  function position(e: DragEvent): DropPosition {
    const rect = node.getBoundingClientRect();
    return e.clientY < rect.top + rect.height / 2 ? "before" : "after";
  }

  function onDragStart(e: DragEvent) {
    if (p.disabled) { e.preventDefault(); return; }
    dragging = { group: p.group, index: p.index };
    node.classList.add("dragging");
    e.dataTransfer?.setData("text/plain", String(p.index));
    if (e.dataTransfer) e.dataTransfer.effectAllowed = "move";
  }

  function onDragOver(e: DragEvent) {
    if (!dragging || dragging.group !== p.group || dragging.index === p.index) return;
    e.preventDefault();
    if (e.dataTransfer) e.dataTransfer.dropEffect = "move";
    const pos = position(e);
    node.classList.toggle("drag-over-top", pos === "before");
    node.classList.toggle("drag-over-bottom", pos === "after");
  }

  function onDragLeave() {
    clearOver();
  }

  function onDrop(e: DragEvent) {
    if (!dragging || dragging.group !== p.group) return;
    e.preventDefault();
    const from = dragging.index;
    clearOver();
    if (from !== p.index) p.onDrop(from, p.index, position(e));
    dragging = null;
  }

  function onDragEnd() {
    node.classList.remove("dragging");
    clearOver();
    dragging = null;
  }

  node.draggable = !p.disabled;
  node.addEventListener("dragstart", onDragStart);
  node.addEventListener("dragover", onDragOver);
  node.addEventListener("dragleave", onDragLeave);
  node.addEventListener("drop", onDrop);
  node.addEventListener("dragend", onDragEnd);

  return {
    update(next: ReorderableParams) {
      p = next;
      node.draggable = !p.disabled;
    },
    destroy() {
      node.removeEventListener("dragstart", onDragStart);
      node.removeEventListener("dragover", onDragOver);
      node.removeEventListener("dragleave", onDragLeave);
      node.removeEventListener("drop", onDrop);
      node.removeEventListener("dragend", onDragEnd);
    },
  };
}
