use std::path::PathBuf;

use base64::Engine;
use serde::Serialize;
use tauri::{AppHandle, State};
use ts_rs::TS;

use super::CommandResult;
use crate::app::effects;
use crate::app::state::AppState;
use crate::customizations::files::{self, FileError};
use crate::customizations::{AssetKind, Scope, account_key, scope_dir, toggle};
use crate::i18n::tr;
use crate::settings::AssetToggle;

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct CustomFile {
    pub name: String,
    pub enabled: bool,
    /// Preview image file name (CSS only).
    pub preview: Option<String>,
}

fn kind_dir(state: &AppState, scope: Scope, kind: AssetKind) -> PathBuf {
    scope_dir(&state.paths.customizations_dir(), scope).join(kind.ext())
}

fn previews_dir(state: &AppState, scope: Scope) -> PathBuf {
    scope_dir(&state.paths.customizations_dir(), scope).join("previews/css")
}

/// Change one scope's toggle (file flags and preview map).
fn update_toggle(
    app: &AppHandle,
    scope: Scope,
    kind: AssetKind,
    f: impl FnOnce(&mut AssetToggle),
) -> CommandResult<()> {
    effects::change_settings(app, |s| {
        let c = &mut s.customizations;
        let (css, js) = match scope {
            None => (&mut c.global.css, &mut c.global.js),
            Some(id) => {
                let account = c.accounts.entry(account_key(id)).or_default();
                (&mut account.css, &mut account.js)
            }
        };
        f(match kind {
            AssetKind::Css => css,
            AssetKind::Js => js,
        });
    })?;
    Ok(())
}

fn user_error(err: FileError) -> super::CommandError {
    tr(&err.to_string()).into()
}

#[tauri::command]
pub fn custom_list(state: State<'_, AppState>, scope: Scope, kind: AssetKind) -> Vec<CustomFile> {
    let t = toggle(&state.settings.get().customizations, scope, kind);
    files::list(&kind_dir(&state, scope, kind), kind)
        .into_iter()
        .map(|name| CustomFile {
            enabled: !t.disabled_files.contains(&name),
            preview: t.preview_map.get(&name).cloned(),
            name,
        })
        .collect()
}

#[tauri::command]
pub fn custom_read(
    state: State<'_, AppState>,
    scope: Scope,
    kind: AssetKind,
    name: String,
) -> CommandResult<String> {
    files::read(&kind_dir(&state, scope, kind), &name).map_err(user_error)
}

/// Create (`old_name` absent) or save an edit; a rename keeps the file's
/// enabled flag and preview.
#[tauri::command]
pub fn custom_write(
    app: AppHandle,
    state: State<'_, AppState>,
    scope: Scope,
    kind: AssetKind,
    old_name: Option<String>,
    name: String,
    content: String,
) -> CommandResult<String> {
    let dir = kind_dir(&state, scope, kind);
    let saved =
        files::write(&dir, kind, old_name.as_deref(), &name, &content).map_err(user_error)?;
    if let Some(old) = old_name.filter(|old| *old != saved) {
        let renamed = saved.clone();
        update_toggle(&app, scope, kind, |t| {
            if let Some(i) = t.disabled_files.iter().position(|n| *n == old) {
                t.disabled_files[i] = renamed.clone();
                t.disabled_files.sort();
            }
            if let Some(preview) = t.preview_map.remove(&old) {
                t.preview_map.insert(renamed, preview);
            }
        })?;
    }
    Ok(saved)
}

#[tauri::command]
pub async fn custom_import_file(
    app: AppHandle,
    state: State<'_, AppState>,
    scope: Scope,
    kind: AssetKind,
) -> CommandResult<Option<String>> {
    use tauri_plugin_dialog::DialogExt;
    let picked = app
        .dialog()
        .file()
        .set_title(tr("Select file"))
        .add_filter(format!("*.{}", kind.ext()), &[kind.ext()])
        .blocking_pick_file()
        .and_then(|p| p.into_path().ok());
    let Some(source) = picked else {
        return Ok(None);
    };
    let name =
        files::import_file(&kind_dir(&state, scope, kind), kind, &source).map_err(user_error)?;
    Ok(Some(name))
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct UrlImport {
    pub name: String,
    /// Whether a userstyles.world preview was attached.
    pub preview: bool,
}

#[tauri::command]
pub async fn custom_import_url(
    app: AppHandle,
    state: State<'_, AppState>,
    scope: Scope,
    kind: AssetKind,
    url: String,
    name: Option<String>,
) -> CommandResult<UrlImport> {
    let dir = kind_dir(&state, scope, kind);
    let saved = files::import_url(&dir, kind, &url, name.as_deref())
        .await
        .map_err(|err| match err {
            FileError::Http(_) | FileError::Io(_) => match kind {
                AssetKind::Css => tr("Could not download CSS from URL.").into(),
                AssetKind::Js => tr("Could not download JavaScript from URL.").into(),
            },
            other => user_error(other),
        })?;
    let mut preview = false;
    if kind == AssetKind::Css
        && let Some(id) = files::userstyles_world_id(&url)
        && let Some(image) =
            files::fetch_userstyles_preview(&previews_dir(&state, scope), &saved, id).await
    {
        let css = saved.clone();
        update_toggle(&app, scope, kind, |t| {
            t.preview_map.insert(css, image);
        })?;
        preview = true;
    }
    Ok(UrlImport {
        name: saved,
        preview,
    })
}

#[tauri::command]
pub fn custom_duplicate(
    state: State<'_, AppState>,
    scope: Scope,
    kind: AssetKind,
    name: String,
) -> CommandResult<String> {
    files::duplicate(&kind_dir(&state, scope, kind), kind, &name).map_err(user_error)
}

/// Delete a file with its flag and preview (the image goes too when no
/// other file uses it).
#[tauri::command]
pub fn custom_delete(
    app: AppHandle,
    state: State<'_, AppState>,
    scope: Scope,
    kind: AssetKind,
    name: String,
) -> CommandResult<()> {
    files::delete(&kind_dir(&state, scope, kind), &name).map_err(user_error)?;
    let mut orphan = None;
    update_toggle(&app, scope, kind, |t| {
        t.disabled_files.retain(|n| *n != name);
        if let Some(image) = t.preview_map.remove(&name)
            && !t.preview_map.values().any(|v| *v == image)
        {
            orphan = Some(image);
        }
    })?;
    if let Some(image) = orphan {
        let _ = files::delete(&previews_dir(&state, scope), &image);
    }
    Ok(())
}

#[tauri::command]
pub fn custom_set_file_enabled(
    app: AppHandle,
    scope: Scope,
    kind: AssetKind,
    name: String,
    enabled: bool,
) -> CommandResult<()> {
    update_toggle(&app, scope, kind, |t| {
        t.disabled_files.retain(|n| *n != name);
        if !enabled {
            t.disabled_files.push(name);
            t.disabled_files.sort();
        }
    })
}

#[tauri::command]
pub fn custom_open_folder(
    state: State<'_, AppState>,
    scope: Scope,
    kind: AssetKind,
) -> CommandResult<()> {
    let dir = kind_dir(&state, scope, kind);
    std::fs::create_dir_all(&dir)?;
    tauri_plugin_opener::open_path(&dir, None::<&str>)?;
    Ok(())
}

#[tauri::command]
pub async fn custom_pick_preview(
    app: AppHandle,
    state: State<'_, AppState>,
    scope: Scope,
    name: String,
) -> CommandResult<Option<String>> {
    use tauri_plugin_dialog::DialogExt;
    let picked = app
        .dialog()
        .file()
        .set_title(tr("Select preview image"))
        .add_filter(tr("Images"), &["png", "jpg", "jpeg", "webp", "gif", "bmp"])
        .blocking_pick_file()
        .and_then(|p| p.into_path().ok());
    let Some(source) = picked else {
        return Ok(None);
    };
    let previews = previews_dir(&state, scope);
    let image = files::set_preview(&previews, &name, &source).map_err(user_error)?;
    let mut old = None;
    let new_image = image.clone();
    update_toggle(&app, scope, AssetKind::Css, |t| {
        old = t.preview_map.insert(name, new_image);
        if old
            .as_ref()
            .is_some_and(|o| t.preview_map.values().any(|v| v == o))
        {
            old = None;
        }
    })?;
    if let Some(old) = old.filter(|o| *o != image) {
        let _ = files::delete(&previews, &old);
    }
    Ok(Some(image))
}

/// A preview image as a data URL; a mapping to a missing file is dropped.
#[tauri::command]
pub fn custom_preview(
    app: AppHandle,
    state: State<'_, AppState>,
    scope: Scope,
    name: String,
) -> CommandResult<Option<String>> {
    let t = toggle(&state.settings.get().customizations, scope, AssetKind::Css);
    let Some(image) = t.preview_map.get(&name) else {
        return Ok(None);
    };
    let path = files::safe_join(&previews_dir(&state, scope), image).map_err(user_error)?;
    let Ok(bytes) = std::fs::read(&path) else {
        update_toggle(&app, scope, AssetKind::Css, |t| {
            t.preview_map.remove(&name);
        })?;
        return Ok(None);
    };
    let mime = match path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_lowercase)
        .as_deref()
    {
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("webp") => "image/webp",
        Some("gif") => "image/gif",
        Some("bmp") => "image/bmp",
        _ => "image/png",
    };
    Ok(Some(format!(
        "data:{mime};base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    )))
}

/// Reload the scope's page (the active account for the global scope), which
/// injects the customizations again.
#[tauri::command]
pub fn custom_reload(app: AppHandle, state: State<'_, AppState>, scope: Scope) {
    if let Some(id) = scope.or_else(|| state.webviews.active()) {
        state.webviews.reload(&app, id);
    }
}
