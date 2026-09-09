fn main() {
    // **LE SCRIPT DE TAURI NE TOURNE QUE SI TAURI EST LA.** Il panique sinon
    // (« missing cargo:dev instruction »), ce qui empeche meme de MESURER ce qui reste a
    // decrocher : la compilation s'arrete avant d'avoir lu une seule ligne de code.
    #[cfg(feature = "interface-tauri")]
    tauri_build::build();
}
