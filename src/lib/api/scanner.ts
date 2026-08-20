import { invoke } from "@tauri-apps/api/core";
import type { ScanResult, DbProject, ProjectFolder } from "../types";

export const scanDir = (path: string) => invoke<ScanResult>("scan_dir", { path });
export const scanSubdirs = (path: string) => invoke<ScanResult[]>("scan_subdirs", { path });
export const getDbProjects = () => invoke<DbProject[]>("get_db_projects");
export const addProject = (name: string, path: string, composeFile: string, description: string, dependsOn: string[]) =>
  invoke<DbProject>("add_project", { name, path, composeFile, description, dependsOn });
export const updateDbProject = (id: number, name: string, path: string, composeFile: string, description: string, dependsOn: string[]) =>
  invoke<DbProject>("update_db_project", { id, name, path, composeFile, description, dependsOn });
export const deleteDbProject = (id: number) => invoke("delete_db_project", { id });
export const reorderProjects = (names: string[]) => invoke("reorder_projects", { names });
export const getProjectSettings = (name: string) => invoke<DbProject>("get_project_settings", { name });
export const renameProject = (oldName: string, newName: string) => invoke("rename_project", { oldName, newName });
export const updateProjectSettings = (name: string, path: string, composeFile: string, description: string, dependsOn: string[]) =>
  invoke<DbProject>("update_project_settings", { name, path, composeFile, description, dependsOn });

// Project Folders
export const getProjectFolders = () => invoke<ProjectFolder[]>("get_project_folders");
export const createProjectFolder = (name: string, parentId: number | null = null) =>
  invoke<ProjectFolder>("create_project_folder", { name, parentId });
export const renameProjectFolder = (id: number, name: string) => invoke("rename_project_folder", { id, name });
export const deleteProjectFolder = (id: number) => invoke("delete_project_folder", { id });
/** Reordonne UNE fratrie : passer les ids d'un meme parent, dans l'ordre voulu. */
export const reorderProjectFolders = (ids: number[]) => invoke("reorder_project_folders", { ids });
/** Range un dossier dans un autre (null = racine). Le backend refuse les boucles. */
export const moveProjectFolder = (id: number, parentId: number | null) =>
  invoke("move_project_folder", { id, parentId });
export const moveProjectToFolder = (projectName: string, folderId: number | null) => invoke("move_project_to_folder", { projectName, folderId });
