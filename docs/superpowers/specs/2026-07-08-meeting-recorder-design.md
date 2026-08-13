# Enregistrement de réunion — Design

Date : 2026-07-08 · Statut : validé

## Objectif

Bouton Record par projet. Capture micro + son système, transcription Whisper de qualité,
résumé LLM piloté par un prompt système éditable, le tout déposé automatiquement dans une
note du projet.

## Pipeline

```
⏺ Record (2 pistes pw-record) → ⏹ Stop → Transcription whisper-1 (par piste, chunks 10 min)
→ Fusion chronologique "Moi / Eux" → Résumé chat completions (prompt éditable)
→ Note "Réunion du JJ/MM/AAAA à HHhMM" dans le dossier "Réunions" du projet
```

## Décisions

| Sujet | Choix |
|-------|-------|
| Transcription | API OpenAI `whisper-1`, `verbose_json` (timestamps), langue fr |
| Capture | 2 pistes séparées : micro (source défaut) + son système (monitor du sink défaut, PipeWire `stream.capture.sink=true`) |
| Format | PCM brut s16 mono 16 kHz (stdout pw-record → fichier .raw), WAV reconstruit en mémoire par chunk |
| Chunks | 600 s (~19,2 Mo < limite API 25 Mo), timestamps décalés par offset de chunk |
| Diarisation | Partielle : piste micro = "Moi", piste système = "Eux", fusion par timestamps |
| Résumé | OpenAI chat completions, modèle configurable (défaut `gpt-4o`) |
| Prompt système | Global éditable (Paramètres globaux) + override par projet (Paramètres projet) |
| Sortie | Note Markdown auto-créée dans dossier "Réunions" du projet : résumé puis transcription dialoguée |
| Audio | Supprimé après succès ; conservé si échec, bouton "Réessayer" |
| Concurrence | Un seul enregistrement à la fois (global) |
| Clé API | Table `settings`, importée au démarrage depuis `~/.local/share/com.cockpit.dev/secrets.json` si absente |

## Backend (Rust)

Nouveau module `src-tauri/src/recorder/` :

- `capture.rs` — spawn 2 `pw-record` (stdout → .raw), stop via SIGTERM, mesure durée
- `wav.rs` — découpe le PCM brut en chunks, fabrique les WAV en mémoire (header 44 octets)
- `transcribe.rs` — upload multipart vers `/v1/audio/transcriptions`, parse `verbose_json`,
  filtre les segments `no_speech_prob` élevés, skip les chunks quasi silencieux (amplitude max)
- `summarize.rs` — `/v1/chat/completions` (system = prompt, user = transcription)
- `mod.rs` — machine à états (recording → transcribing → summarizing → done | error),
  commandes Tauri, events `recording_status`, création de la note, cleanup audio

Storage :

- `storage/settings.rs` — table `settings` clé/valeur (`openai_api_key`, `summary_prompt`, `summary_model`)
- `storage/recordings.rs` — table `recordings` (id, project_id, started_at, duration_secs, state, error, dir)
- Migration : + colonne `summary_prompt` nullable sur `projects`

Commandes Tauri : `start_recording`, `stop_recording`, `get_active_recording`,
`get_failed_recordings`, `retry_recording`, `delete_recording`,
`get_app_settings`, `set_app_setting`.

Event : `recording_status` `{ recording_id, project_id, state, error? }`.

## Frontend (Svelte 5)

- `stores/recording.ts` — état courant (alimenté par `get_active_recording` + event `recording_status`)
- `api/recorder.ts` — wrappers invoke
- `ProjectDetail.svelte` — bouton ⏺ Enregistrer dans l'en-tête → timer rouge + ⏹ Stop,
  puis badge "Transcription…" / "Résumé…" ; bandeau retry si enregistrements en échec
- `GlobalSettings.svelte` — section "Réunions" : clé API, prompt système, modèle
- `SettingsTab.svelte` — textarea override du prompt (vide = prompt global)
- Rechargement de l'arborescence notes quand `state === "done"`

## Format de la note

```markdown
# Réunion du 08/07/2026 à 14h30

*Durée : 42 min*

## Résumé

<sortie LLM>

## Transcription

**Moi** [00:12] : ...
**Eux** [00:45] : ...
```

## Limites assumées

- Pas de noms côté distant : tout ce qui vient du son système est "Eux"
- Léger risque de désynchro < 1 s entre pistes (démarrage quasi simultané des 2 pw-record)
- Whisper peut halluciner sur du silence → double filtre (amplitude + no_speech_prob)
