//! Ce que le backend dit a l'interface, sans savoir laquelle l'ecoute.
//!
//! **POURQUOI CE TRAIT EXISTE.** L'implementation des terminaux recevait un `AppHandle` et
//! le gardait pour emettre la sortie. C'etait le SEUL lien entre le coeur du backend et
//! Tauri sur ce chemin, et il suffisait a rendre le reste inutilisable ailleurs. Le trait
//! le remplace : Tauri en est une implementation parmi d'autres, au meme titre que la
//! coquille qui ecrit sur son tuyau.
//!
//! **LA CHARGE PASSE PAR `serde_json::Value`, ET C'EST MESURE.** Une methode generique
//! rendrait le trait inutilisable derriere un `dyn`. Le cout d'une `Value` serait
//! discutable sur un chemin appele a chaque octet ; il ne l'est pas ici, la sortie partant
//! deja en gros lots (33 a 54 envois pour 1,3 Mo de sortie mesuree).

use std::sync::Arc;

/// Emet un evenement vers l'interface. Une panne d'emission ne remonte pas : il n'y a rien
/// a faire d'utile si personne n'ecoute, et le chemin est trop chaud pour y ajouter un
/// traitement.
pub trait Emetteur: Send + Sync {
    fn emettre(&self, evenement: &str, charge: serde_json::Value);
}

/// L'emetteur tel qu'on le fait circuler. `Arc` parce que plusieurs fils l'utilisent :
/// la boucle qui lit le service de terminaux, et celle qui les rebranche apres coupure.
pub type Emetteurs = Arc<dyn Emetteur>;

impl Emetteur for tauri::AppHandle {
    fn emettre(&self, evenement: &str, charge: serde_json::Value) {
        use tauri::Emitter;
        let _ = self.emit(evenement, charge);
    }
}
