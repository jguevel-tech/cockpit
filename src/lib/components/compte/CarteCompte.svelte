<script lang="ts">
  /**
   * La carte « Compte » des reglages : elle DIT l'etat, et renvoie a la page du compte.
   *
   * Elle portait avant les memes commandes que le profil — etat de la synchronisation, bouton
   * de synchronisation, deconnexion, adresse du serveur. Deux ecrans pour la meme chose font
   * chercher la difference qui n'existe pas, et obligent a se souvenir lequel porte quoi. Tout
   * cela vit maintenant dans la page du compte ; il reste ici de quoi savoir ou on en est sans
   * la quitter des yeux, et un chemin pour y aller.
   *
   * Elle dit toujours ce qu'il en est, y compris quand il n'y a pas de compte — un ecran qui se
   * contente de ne rien afficher laisse croire a une panne.
   */
  import { trad } from "../../i18n";
  import { compte } from "../../stores/compte";
  import { openCompte } from "../../stores/ui";
</script>

<section class="card">
  <div class="card-head">
    <h3>{$trad("settings.compte.title")}</h3>
    <p>{$trad("settings.compte.help")}</p>
  </div>

  {#if $compte?.connecte}
    <div class="field-row">
      <span class="field-label">{$trad("settings.compte.connecteEnTant")}</span>
      <span class="field-value">{$compte.email}</span>
    </div>
    <div class="actions">
      <button class="btn" onclick={openCompte}>{$trad("settings.compte.ouvrirLaPage")}</button>
    </div>
  {:else}
    <p class="etat">{$trad("settings.compte.aucun")}</p>
    <div class="actions">
      <button class="btn primary" onclick={openCompte}>
        {$trad("settings.compte.seConnecter")}
      </button>
    </div>
  {/if}
</section>

<style>
  .etat {
    margin: 0 0 0.9rem;
    color: var(--text-secondary);
    font-size: 0.86rem;
  }
  .actions {
    display: flex;
    gap: 0.5rem;
    margin-top: 0.9rem;
  }
</style>
