use tauri::{AppHandle, State};

use super::CommandResult;
use crate::app::effects;
use crate::app::state::AppState;
use crate::i18n::tr;
use crate::spellcheck::{self, SpellState, push_recent, store};

fn user_error(err: store::SpellError) -> super::CommandError {
    tr(&err.to_string()).into()
}

#[tauri::command]
pub fn spell_state(app: AppHandle) -> SpellState {
    spellcheck::state(&app)
}

/// Apply the language picker: store the selection and update the recents.
#[tauri::command]
pub fn spell_set_languages(app: AppHandle, languages: Vec<String>) -> CommandResult<()> {
    let previous = spellcheck::state(&app).languages;
    effects::change_settings(&app, |s| {
        let sp = &mut s.spellcheck;
        sp.recent_languages = push_recent(&sp.recent_languages, &previous, &languages);
        sp.languages = languages;
    })?;
    Ok(())
}

#[tauri::command]
pub async fn spell_install(app: AppHandle, tag: String) -> CommandResult<()> {
    spellcheck::install(&app, &tag).await.map_err(user_error)
}

/// Remove a FastZap dictionary; `disable` turns the checker off first when
/// it was the last language in use.
#[tauri::command]
pub fn spell_remove(
    app: AppHandle,
    state: State<'_, AppState>,
    tag: String,
    disable: bool,
) -> CommandResult<()> {
    store::remove(&state.paths.dictionaries_dir(), &tag).map_err(user_error)?;
    effects::change_settings(&app, |s| {
        s.spellcheck.languages.retain(|l| *l != tag);
        s.spellcheck.recent_languages.retain(|l| *l != tag);
        if disable {
            s.spellcheck.enabled = false;
        }
    })?;
    spellcheck::apply_all(&app);
    Ok(())
}

/// Import a `.dic` with the `.aff` beside it. Returns the installed tag.
#[tauri::command]
pub async fn spell_import(
    app: AppHandle,
    state: State<'_, AppState>,
) -> CommandResult<Option<String>> {
    use tauri_plugin_dialog::DialogExt;
    let picked = app
        .dialog()
        .file()
        .set_title(tr("Import dictionaries"))
        .add_filter(tr("Hunspell dictionaries (*.dic)"), &["dic"])
        .blocking_pick_file()
        .and_then(|p| p.into_path().ok());
    let Some(dic) = picked else {
        return Ok(None);
    };
    let tag = store::import(&state.paths.dictionaries_dir(), &dic).map_err(user_error)?;
    spellcheck::apply_all(&app);
    Ok(Some(tag))
}

#[tauri::command]
pub fn spell_open_folder(state: State<'_, AppState>) -> CommandResult<()> {
    let dir = store::managed_dir(&state.paths.dictionaries_dir());
    std::fs::create_dir_all(&dir)?;
    tauri_plugin_opener::open_path(&dir, None::<&str>)?;
    Ok(())
}
