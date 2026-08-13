declare const __BUILD_TIME__: string;

// Sous-chemins resolus par Vite a runtime mais sans declarations de types publiees
declare module "@shikijs/langs/*";
declare module "@shikijs/themes/*";
declare module "shiki/core";
declare module "shiki/engine/oniguruma";
declare module "shiki/wasm";
declare module "@xterm/xterm/css/xterm.css";
