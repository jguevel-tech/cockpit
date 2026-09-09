// Le pont, et le SEUL point de contact entre la page et le reste du monde.
//
// La page tourne dans le bac a sable : elle n'a ni Node, ni acces au processus principal.
// Tout ce qu'elle peut faire est ce qui est expose ici, nommement. C'est l'equivalent de la
// liste des commandes de Tauri : ce qui n'y figure pas n'existe pas pour la page.

const { contextBridge, ipcRenderer } = require('electron')

contextBridge.exposeInMainWorld('cockpit', {
  /**
   * Appelle une commande du backend. Remplace `invoke` de Tauri, meme forme d'appel et
   * meme contrat : une commande nommee, des arguments serialisables, une promesse qui
   * porte le resultat ou rejette avec le message d'erreur.
   *
   * Le nom est passe tel quel au processus principal, qui tient la liste des commandes
   * connues et REFUSE le reste : filtrer ici ne protegerait de rien, la page pouvant
   * appeler ce pont avec ce qu'elle veut.
   */
  invoke: (commande, arguments_) => ipcRenderer.invoke('cockpit:commande', commande, arguments_),

  /**
   * Abonne la page a un evenement pousse par le backend (sortie de terminal, fin de
   * processus, etat de la synchro). Rend la fonction qui coupe l'abonnement.
   *
   * L'ecouteur ne recoit QUE la charge utile, jamais l'objet evenement d'Electron : celui-ci
   * porte `sender`, donc un chemin vers le processus principal que la page n'a pas a voir.
   */
  ecouter: (evenement, ecouteur) => {
    const enveloppe = (_, charge) => ecouteur(charge)
    ipcRenderer.on(`cockpit:evenement:${evenement}`, enveloppe)
    return () => ipcRenderer.off(`cockpit:evenement:${evenement}`, enveloppe)
  }
})
