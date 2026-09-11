//! Les fournisseurs declares.
//!
//! **La plupart ne sont qu'une declaration** : un identifiant, un nom, le nom de leur CLI, un
//! symbole et une couleur. Ca suffit pour qu'un agent qui tourne dans un terminal soit reconnu
//! et pour qu'on puisse les choisir. Ceux qui savent faire davantage ont leur propre fichier et
//! implementent les traits correspondants.
//!
//! Les symboles sont des CARACTERES et non des logos : un logo par fournisseur voudrait dire un
//! fichier a fournir et un droit d'usage a verifier pour chaque nouveau venu. Ils restent
//! distincts les uns des autres, c'est tout ce qu'on leur demande.

pub mod claude;
pub mod openai;

use super::Declaration;

pub use openai::OPENAI;

pub static CODEX: Declaration = Declaration {
    id: "codex",
    nom: "Codex",
    commandes: &["codex"],
    symbole: "◆",
    couleur: "#10a37f",
};

pub static GEMINI: Declaration = Declaration {
    id: "gemini",
    nom: "Gemini",
    commandes: &["gemini"],
    symbole: "◇",
    couleur: "#4285f4",
};

pub static AIDER: Declaration = Declaration {
    id: "aider",
    nom: "Aider",
    commandes: &["aider"],
    symbole: "▲",
    couleur: "#14b8a6",
};

pub static GOOSE: Declaration = Declaration {
    id: "goose",
    nom: "Goose",
    commandes: &["goose"],
    symbole: "▼",
    couleur: "#b45309",
};

pub static OPENCODE: Declaration = Declaration {
    id: "opencode",
    nom: "OpenCode",
    commandes: &["opencode"],
    symbole: "◈",
    couleur: "#7c3aed",
};

pub static COPILOT: Declaration = Declaration {
    id: "copilot",
    nom: "Copilot",
    commandes: &["copilot"],
    symbole: "◐",
    // La couleur de marque est un noir bleute, INVISIBLE sur le theme sombre : le symbole
    // disparaissait. Une couleur doit rester lisible sur les DEUX themes — vu au rendu.
    couleur: "#8b949e",
};

pub static CURSOR: Declaration = Declaration {
    id: "cursor",
    nom: "Cursor",
    commandes: &["cursor-agent"],
    symbole: "▸",
    couleur: "#0ea5e9",
};

pub static AMP: Declaration = Declaration {
    id: "amp",
    nom: "Amp",
    commandes: &["amp"],
    symbole: "◉",
    couleur: "#f59e0b",
};

pub static QWEN: Declaration = Declaration {
    id: "qwen",
    nom: "Qwen",
    commandes: &["qwen"],
    symbole: "◎",
    couleur: "#615ced",
};

pub static OLLAMA: Declaration = Declaration {
    id: "ollama",
    nom: "Ollama",
    commandes: &["ollama"],
    symbole: "○",
    // Meme raison que Copilot : le noir de la marque ne se voit pas sur fond sombre.
    couleur: "#c9d1d9",
};
