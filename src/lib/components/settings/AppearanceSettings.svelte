<script lang="ts">
  import { open } from "@tauri-apps/plugin-dialog";
  import {
    THEMES, theme, accent, wallpaper, surfaceAlpha, wallpaperDim, wallpaperBlur,
    applyWallpaper, removeWallpaper, resetAppearance,
  } from "../../stores/appearance";
  import { readImageAsDataUrl } from "../../api/appearance";
  import { notify } from "../../stores/toast";

  let busy = $state(false);
  let deriveAccent = $state(true);

  async function pickImage() {
    if (busy) return;
    try {
      const path = await open({
        multiple: false,
        directory: false,
        filters: [{ name: "Images", extensions: ["webp", "jpg", "jpeg", "png"] }],
      });
      if (typeof path !== "string") return;

      busy = true;
      // Rust lit et valide le fichier ; le frontend redimensionne et echantillonne la
      // couleur dominante (c'est lui qui a un canvas).
      await applyWallpaper(await readImageAsDataUrl(path), deriveAccent);
      notify("Image de fond appliquée", "success");
    } catch (e) {
      notify(String(e));
    } finally {
      busy = false;
    }
  }
</script>

<section class="card">
  <div class="card-head">
    <h3>Thème</h3>
    <p>La palette s'applique à toute l'interface. Le terminal suit la base claire ou sombre.</p>
  </div>
  <div class="themes">
    {#each THEMES as t}
      <button class="swatch" class:active={$theme === t.id} onclick={() => theme.set(t.id)} title={t.label}>
        <span class="chips">
          {#each t.preview as c}<span class="chip" style:background={c}></span>{/each}
        </span>
        <span class="swatch-label">{t.label}</span>
      </button>
    {/each}
  </div>
</section>

<section class="card">
  <div class="card-head">
    <h3>Couleur d'accent</h3>
    <p>Remplace l'accent de la palette. Utilisée pour les boutons, les liens et les états actifs.</p>
  </div>
  <div class="inline-row">
    <input
      type="color"
      class="color-input"
      value={$accent ?? THEMES.find((t) => t.id === $theme)?.preview[2] ?? "#6d8dff"}
      oninput={(e) => accent.set(e.currentTarget.value)}
      aria-label="Couleur d'accent"
    />
    <code class="mono-value">{$accent ?? "accent de la palette"}</code>
    {#if $accent}
      <button class="btn small" onclick={() => accent.set(null)}>Réinitialiser</button>
    {/if}
  </div>
</section>

<section class="card">
  <div class="card-head">
    <h3>Image de fond</h3>
    <p>
      L'image est redimensionnée et recompressée à l'import. Les surfaces deviennent translucides
      et floutées pour que le texte reste lisible — le terminal, lui, reste opaque.
    </p>
  </div>

  {#if $wallpaper}
    <div class="preview" style:background-image="url({$wallpaper})"></div>
  {/if}

  <div class="inline-row">
    <button class="btn primary" onclick={pickImage} disabled={busy}>
      {busy ? "Traitement…" : $wallpaper ? "Changer l'image" : "Choisir une image"}
    </button>
    {#if $wallpaper}
      <button class="btn danger" onclick={removeWallpaper}>Retirer</button>
    {/if}
  </div>

  <label class="check">
    <input type="checkbox" bind:checked={deriveAccent} />
    <span>Reprendre la couleur dominante de l'image comme accent</span>
  </label>

  {#if $wallpaper}
    <div class="sliders">
      <label>
        <span class="slider-label">Voile <em>{Math.round($wallpaperDim * 100)} %</em></span>
        <input type="range" min="0" max="95" value={$wallpaperDim * 100}
          oninput={(e) => wallpaperDim.set(Number(e.currentTarget.value) / 100)} />
        <span class="hint">Plus le voile est fort, plus le texte est lisible et l'image discrète.</span>
      </label>
      <label>
        <span class="slider-label">Flou de l'image <em>{$wallpaperBlur} px</em></span>
        <input type="range" min="0" max="24" value={$wallpaperBlur}
          oninput={(e) => wallpaperBlur.set(Number(e.currentTarget.value))} />
        <span class="hint">Un léger flou calme les images très détaillées.</span>
      </label>
      <label>
        <span class="slider-label">Opacité des surfaces <em>{$surfaceAlpha} %</em></span>
        <input type="range" min="40" max="100" value={$surfaceAlpha}
          oninput={(e) => surfaceAlpha.set(Number(e.currentTarget.value))} />
        <span class="hint">À 100 %, les panneaux redeviennent opaques et l'image n'apparaît qu'en fond.</span>
      </label>
    </div>
  {/if}
</section>

<section class="card">
  <div class="card-head">
    <h3>Réinitialiser</h3>
    <p>Revient au thème sombre par défaut, sans accent personnalisé. L'image n'est pas supprimée.</p>
  </div>
  <button class="btn" onclick={resetAppearance}>Réinitialiser l'apparence</button>
</section>

<style>
  .themes { display: flex; flex-wrap: wrap; gap: 0.75rem; }
  .swatch {
    display: flex; flex-direction: column; gap: 0.5rem; align-items: center;
    padding: 0.6rem 0.7rem;
    background: none; cursor: pointer;
    border: 1px solid var(--border-color); border-radius: var(--radius);
    color: var(--text-secondary); font-family: inherit; font-size: 0.75rem;
    transition: border-color 0.12s ease, color 0.12s ease;
  }
  .swatch:hover { border-color: var(--border-strong); color: var(--text-primary); }
  .swatch.active { border-color: var(--accent); color: var(--text-primary); }
  .chips { display: flex; border-radius: var(--radius-sm); overflow: hidden; }
  .chip { width: 18px; height: 26px; }
  .swatch-label { font-weight: 500; }

  .color-input {
    width: 42px; height: 30px; padding: 0;
    background: none; border: 1px solid var(--border-color);
    border-radius: var(--radius-sm); cursor: pointer;
  }

  .preview {
    height: 110px; margin-bottom: 0.75rem;
    background-size: cover; background-position: center;
    border: 1px solid var(--border-color); border-radius: var(--radius);
  }

  .check {
    display: flex; align-items: center; gap: 0.45rem;
    margin-top: 0.75rem; font-size: 0.83rem; color: var(--text-secondary);
    cursor: pointer;
  }

  /* Chaque reglage est un bloc separe : titre, curseur, explication. Sans cet espacement,
     les trois curseurs et leurs legendes se melangeaient visuellement. */
  .sliders { display: flex; flex-direction: column; gap: 1.5rem; margin-top: 1.5rem; }
  .sliders label { display: flex; flex-direction: column; gap: 0.5rem; }
  .slider-label { font-size: 0.83rem; color: var(--text-primary); }
  .slider-label em { color: var(--text-secondary); font-style: normal; font-variant-numeric: tabular-nums; }
  .sliders input[type="range"] { width: 100%; accent-color: var(--accent); margin: 0; }
  .hint { font-size: 0.76rem; color: var(--text-muted); line-height: 1.45; }
</style>
