import { invoke } from "@tauri-apps/api/core";

/// La page a-t-elle peint depuis son dernier passage, et la fenetre etait-elle visible ?
export const santePage = (aPeint: boolean, visible: boolean) =>
  invoke<void>("sante_page", { aPeint, visible });
