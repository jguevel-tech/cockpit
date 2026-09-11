//! L'attribut `#[commande]`, et rien d'autre.
//!
//! **POURQUOI UN MARQUEUR PLUTOT QUE RIEN.** Le dispatch du pont est GENERE en lisant les
//! fonctions marquees, et le generateur les COMPTE pour refuser de tourner s'il en lit
//! moins qu'il n'y a de marques : c'est ce qui a revele 72 commandes manquantes en 2026-09.
//! Sans marque, il n'y aurait plus rien a compter.
//!
//! **POURQUOI UN ATTRIBUT ET PAS UN COMMENTAIRE.** Un commentaire mal orthographie ne se
//! voit pas ; `#[commmande]` ne compile pas. La marque est donc tenue par le compilateur.
//!
//! L'attribut ne genere aucun code : il rend la fonction telle quelle.

use proc_macro::TokenStream;

#[proc_macro_attribute]
pub fn commande(_attributs: TokenStream, element: TokenStream) -> TokenStream {
    element
}
