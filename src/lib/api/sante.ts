import { invoke } from "@tauri-apps/api/core";

/// Ce que la page a dessine depuis son dernier passage.
export const santePage = (images: number) => invoke<void>("sante_page", { images });
