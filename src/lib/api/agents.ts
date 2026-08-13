import { invoke } from "@tauri-apps/api/core";
import type {
  MarketplaceLocation,
  PluginInfo,
  AgentInfo,
  OrchestratorConfig,
} from "../types";

// --- Marketplaces ---

export const getMarketplacePath = () =>
  invoke<string>("get_marketplace_path");

export const listMarketplaces = () =>
  invoke<MarketplaceLocation[]>("list_marketplaces");

// --- Plugins / agents listing ---

export const listPlugins = (marketplaceId: string) =>
  invoke<PluginInfo[]>("list_plugins", { marketplaceId });

export const listAgents = (marketplaceId: string, plugin: string) =>
  invoke<AgentInfo[]>("list_agents", { marketplaceId, plugin });

// --- Agents CRUD ---

export const readAgent = (marketplaceId: string, plugin: string, name: string) =>
  invoke<string>("read_agent", { marketplaceId, plugin, name });

export const saveAgent = (
  marketplaceId: string,
  plugin: string,
  name: string,
  content: string,
) =>
  invoke<void>("save_agent", { marketplaceId, plugin, name, content });

export const deleteAgent = (marketplaceId: string, plugin: string, name: string) =>
  invoke<void>("delete_agent", { marketplaceId, plugin, name });

export const renameAgent = (
  marketplaceId: string,
  plugin: string,
  oldName: string,
  newName: string,
) => invoke<void>("rename_agent", { marketplaceId, plugin, oldName, newName });

// --- Plugins CRUD ---

export const createPlugin = (name: string, description: string) =>
  invoke<void>("create_plugin", { name, description });

export const deletePlugin = (marketplaceId: string, name: string) =>
  invoke<void>("delete_plugin", { marketplaceId, name });

export const renamePlugin = (
  marketplaceId: string,
  oldName: string,
  newName: string,
) => invoke<void>("rename_plugin", { marketplaceId, oldName, newName });

// --- Per-project plugin activation ---

export const getProjectPlugins = (projectPath: string) =>
  invoke<string[]>("get_project_plugins", { projectPath });

export const setProjectPlugins = (projectPath: string, plugins: string[]) =>
  invoke<void>("set_project_plugins", { projectPath, plugins });

// --- Orchestrator / Claude global settings ---

export const getOrchestratorConfig = () =>
  invoke<OrchestratorConfig>("get_orchestrator_config");

export const setTeamsEnabled = (enabled: boolean) =>
  invoke<void>("set_teams_enabled", { enabled });

export const setTeammateMode = (mode: string) =>
  invoke<void>("set_teammate_mode", { mode });

export const togglePluginEnabled = (pluginKey: string, enabled: boolean) =>
  invoke<void>("toggle_plugin_enabled", { pluginKey, enabled });
