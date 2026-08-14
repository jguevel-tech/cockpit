import { invoke } from "@tauri-apps/api/core";
import type { Todo, Note, NoteTree, NoteFolder, NoteFile, Url, UrlHealth, ProjectCommand } from "../types";

// Todos
export const getTodos = (project: string) => invoke<Todo[]>("get_todos", { project });
export const createTodo = (project: string, text: string) => invoke<Todo>("create_todo", { project, text });
export const updateTodo = (id: number, text: string, done: boolean) => invoke<Todo>("update_todo", { id, text, done });
export const setTodoDue = (id: number, dueDate: string | null) => invoke<Todo>("set_todo_due", { id, dueDate });
export const deleteTodo = (id: number) => invoke("delete_todo", { id });
export const reorderTodos = (ids: number[]) => invoke("reorder_todos", { ids });
export const moveTodo = (id: number, newProject: string) => invoke("move_todo", { id, newProject });
export const getPendingTodos = () => invoke<Todo[]>("get_pending_todos");

// Notes
export const getNote = (project: string) => invoke<Note | null>("get_note", { project });
export const saveNote = (project: string, content: string) => invoke<Note>("save_note", { project, content });
export const getNoteTree = (project: string) => invoke<NoteTree>("get_note_tree", { project });
export const createNoteFolder = (project: string, parentId: number | null, name: string) =>
  invoke<NoteFolder>("create_note_folder", { project, parentId, name });
export const renameNoteFolder = (id: number, name: string) => invoke("rename_note_folder", { id, name });
export const deleteNoteFolder = (id: number) => invoke("delete_note_folder", { id });
export const createNoteFile = (project: string, folderId: number | null, name: string) =>
  invoke<NoteFile>("create_note_file", { project, folderId, name });
export const getNoteFile = (id: number) => invoke<NoteFile>("get_note_file", { id });
export const saveNoteFile = (id: number, content: string) => invoke("save_note_file", { id, content });
export const renameNoteFile = (id: number, name: string) => invoke("rename_note_file", { id, name });
export const deleteNoteFile = (id: number) => invoke("delete_note_file", { id });
export const reorderNoteFolders = (ids: number[]) => invoke("reorder_note_folders", { ids });
export const reorderNoteFiles = (ids: number[]) => invoke("reorder_note_files", { ids });
export const moveNoteFile = (id: number, folderId: number | null) => invoke("move_note_file", { id, folderId });

// URLs
export const getUrls = (project: string) => invoke<Url[]>("get_urls", { project });
export const createUrl = (project: string, label: string, url: string) => invoke<Url>("create_url", { project, label, url });
export const updateUrl = (id: number, label: string, url: string) => invoke<Url>("update_url", { id, label, url });
export const deleteUrl = (id: number) => invoke("delete_url", { id });
export const checkUrls = (urls: string[]) => invoke<UrlHealth[]>("check_urls", { urls });

// Commandes rapides par projet
export const getProjectCommands = (project: string) =>
  invoke<ProjectCommand[]>("get_project_commands", { project });
export const createProjectCommand = (project: string, label: string, command: string) =>
  invoke<ProjectCommand>("create_project_command", { project, label, command });
export const updateProjectCommand = (id: number, label: string, command: string) =>
  invoke<ProjectCommand>("update_project_command", { id, label, command });
export const deleteProjectCommand = (id: number) => invoke("delete_project_command", { id });
export const reorderProjectCommands = (ids: number[]) => invoke("reorder_project_commands", { ids });
