import { invoke } from "@tauri-apps/api/core";

// Image de fond. Stockee en fichier dans <app_data>, pas en base : `get_app_settings()`
// renvoie toutes les cles d'un coup et y glisser des centaines de Ko de base64 alourdirait
// chaque lecture de reglage.
export const setWallpaper = (dataUrl: string) => invoke("set_wallpaper", { dataUrl });
export const getWallpaper = () => invoke<string | null>("get_wallpaper");
export const clearWallpaper = () => invoke("clear_wallpaper");

// Lecture faite en Rust plutot que via @tauri-apps/plugin-fs : le plugin n'est pas installe
// cote JS, et l'ajouter demanderait des permissions de lecture bien plus larges que « une
// image choisie par l'utilisateur ».
export const readImageAsDataUrl = (path: string) => invoke<string>("read_image_as_data_url", { path });
