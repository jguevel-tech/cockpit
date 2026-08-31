import { invoke } from "@tauri-apps/api/core";

/// Ce que la page a dessine depuis son dernier passage, et si la fenetre etait visible.
export const santePage = (images: number, visible: boolean) =>
  invoke<void>("sante_page", { images, visible });
