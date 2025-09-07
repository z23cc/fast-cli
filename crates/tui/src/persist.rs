use std::{fs, io::Write, path::PathBuf};

use anyhow::{Context, Result};
use directories::BaseDirs;
use serde::{Deserialize, Serialize};

use crate::app::{App, Message};

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct SavedState {
    pub sessions: Vec<String>,
    pub current_session: usize,
    pub show_sidebar: bool,
    pub sidebar_scroll: u16,
}

impl From<&App> for SavedState {
    fn from(a: &App) -> Self {
        SavedState {
            sessions: a.sessions.clone(),
            current_session: a.current_session,
            show_sidebar: a.show_sidebar,
            sidebar_scroll: a.sidebar_scroll,
        }
    }
}

pub fn state_path() -> Option<PathBuf> {
    let base = BaseDirs::new()?;
    let dir = base.config_dir().join("fast");
    Some(dir.join("ui_state.json"))
}

pub fn load_state() -> Result<Option<SavedState>> {
    let Some(path) = state_path() else {
        return Ok(None);
    };
    if !path.exists() {
        return Ok(None);
    }
    let data = fs::read(&path).with_context(|| format!("read state file: {}", path.display()))?;
    let s: SavedState = serde_json::from_slice(&data).with_context(|| "parse state json")?;
    Ok(Some(s))
}

pub fn save_state(app: &App) -> Result<()> {
    let Some(path) = state_path() else {
        return Ok(());
    };
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).ok();
    }
    let s: SavedState = app.into();
    let data = serde_json::to_vec_pretty(&s)?;
    let mut tmp = path.clone();
    tmp.set_extension("json.tmp");
    {
        let mut f =
            fs::File::create(&tmp).with_context(|| format!("create tmp: {}", tmp.display()))?;
        f.write_all(&data)?;
        f.flush()?;
    }
    fs::rename(tmp, &path).with_context(|| format!("persist state to {}", path.display()))?;
    Ok(())
}

fn session_dir() -> Option<PathBuf> {
    let base = BaseDirs::new()?;
    let dir = base.data_dir().join("fast").join("sessions");
    Some(dir)
}

fn sanitize(name: &str) -> String {
    let mut s = name
        .trim()
        .replace(['<', '>', ':', '"', '/', '\\', '|', '?', '*'], "_");
    if s.is_empty() {
        s = "default".to_string();
    }
    s
}

fn session_path_for(name: &str) -> Option<PathBuf> {
    let dir = session_dir()?;
    Some(dir.join(format!("{}.jsonl", sanitize(name))))
}

pub fn load_session(name: &str) -> Result<Vec<Message>> {
    let Some(path) = session_path_for(name) else {
        return Ok(Vec::new());
    };
    if !path.exists() {
        return Ok(Vec::new());
    }
    let data = fs::read_to_string(&path)
        .with_context(|| format!("read session file: {}", path.display()))?;
    let mut out = Vec::new();
    for line in data.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Ok(m) = serde_json::from_str::<Message>(line) {
            out.push(m);
        }
    }
    Ok(out)
}

pub fn save_session(name: &str, msgs: &[Message]) -> Result<()> {
    let Some(dir) = session_dir() else {
        return Ok(());
    };
    fs::create_dir_all(&dir).ok();
    let Some(path) = session_path_for(name) else {
        return Ok(());
    };
    let mut tmp = path.clone();
    tmp.set_extension("jsonl.tmp");
    {
        let mut f =
            fs::File::create(&tmp).with_context(|| format!("create tmp: {}", tmp.display()))?;
        for m in msgs {
            let line = serde_json::to_string(m)?;
            f.write_all(line.as_bytes())?;
            f.write_all(b"\n")?;
        }
        f.flush()?;
    }
    fs::rename(tmp, &path).with_context(|| format!("persist session to {}", path.display()))?;
    Ok(())
}

pub fn rename_session(old: &str, new: &str) -> Result<()> {
    let Some(old_path) = session_path_for(old) else {
        return Ok(());
    };
    let Some(new_path) = session_path_for(new) else {
        return Ok(());
    };
    if old_path.exists() {
        if let Some(parent) = new_path.parent() {
            fs::create_dir_all(parent).ok();
        }
        fs::rename(&old_path, &new_path)
            .or_else(|_| {
                fs::copy(&old_path, &new_path)
                    .map(|_| fs::remove_file(&old_path))
                    .map(|_| ())
            })
            .ok();
    }
    Ok(())
}

pub fn delete_session(name: &str) -> Result<()> {
    if let Some(path) = session_path_for(name) {
        if path.exists() {
            let _ = fs::remove_file(path);
        }
    }
    Ok(())
}
