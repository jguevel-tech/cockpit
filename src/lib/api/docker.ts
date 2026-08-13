import { invoke } from "@tauri-apps/api/core";
import type { Project, DockerContainer, DiskUsage, DockerVolume, DockerImage } from "../types";

export const listProjects = () => invoke<Project[]>("list_projects");
export const startProject = (name: string) => invoke("start_project", { name });
export const stopProject = (name: string) => invoke("stop_project", { name });
export const restartProject = (name: string) => invoke("restart_project", { name });

// Vue globale des conteneurs Docker de la machine
export const listAllContainers = () => invoke<DockerContainer[]>("list_all_containers");
export const containerAction = (id: string, action: "start" | "stop" | "restart" | "remove") =>
  invoke("container_action", { id, action });
export const containerActionBulk = (ids: string[], action: "start" | "stop" | "restart" | "remove") =>
  invoke("container_action_bulk", { ids, action });

// Volumes, images, espace disque, prune
export const dockerDiskUsage = () => invoke<DiskUsage[]>("docker_disk_usage");
export const listDockerVolumes = () => invoke<DockerVolume[]>("list_docker_volumes");
export const listDockerImages = () => invoke<DockerImage[]>("list_docker_images");
export const removeDockerVolume = (name: string) => invoke("remove_docker_volume", { name });
export const removeDockerImage = (id: string) => invoke("remove_docker_image", { id });
export const dockerPrune = (target: "containers" | "images" | "images_all" | "volumes" | "builder") =>
  invoke<string>("docker_prune", { target });
