use std::collections::HashMap;
use std::fs;
use std::io::BufRead;
use std::io::BufReader;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;

use chrono::Utc;
use codex_core::rollout::path_utils::slug_for_cwd;
use codex_protocol::ConversationId;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::RolloutLine;
use codex_protocol::protocol::SessionMetaLine;
use color_eyre::Result;
use serde_json::Value;
use tokio::task::spawn_blocking;

/// Ensure every rollout file belongs to a single cwd.
/// If a file contains messages from multiple cwds, split it into separate files,
/// one per cwd, preserving timestamps and data. The original file is kept with
/// a `.mixed.bak` suffix to avoid data loss.
pub async fn normalize_sessions(codex_home: &Path) -> Result<()> {
    let root = codex_home.join("sessions");
    if !root.exists() {
        return Ok(());
    }
    let root = root.canonicalize().unwrap_or(root);
    spawn_blocking(move || normalize_sync(&root)).await??;
    Ok(())
}

fn normalize_sync(root: &Path) -> Result<()> {
    split_mixed_cwds(root)?;
    migrate_into_slug_dirs(root)?;
    Ok(())
}

fn split_mixed_cwds(root: &Path) -> Result<()> {
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(read_dir) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in read_dir.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if path.extension().is_none()
                || !path
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("jsonl"))
            {
                continue;
            }
            split_if_mixed(&path)?;
        }
    }
    Ok(())
}

fn migrate_into_slug_dirs(root: &Path) -> Result<()> {
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(read_dir) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in read_dir.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if path.extension().is_none()
                || !path
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("jsonl"))
            {
                continue;
            }
            migrate_file_if_needed(root, &path)?;
        }
    }
    Ok(())
}

fn split_if_mixed(path: &Path) -> Result<()> {
    let file = match fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return Ok(()),
    };
    let reader = BufReader::new(file);
    let mut groups: HashMap<String, Vec<Value>> = HashMap::new();
    let mut current_cwd: Option<String> = None;
    let mut first_ts: Option<String> = None;

    for line in reader.lines().map_while(Result::ok) {
        let Ok(mut val) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        if first_ts.is_none() {
            first_ts = val
                .get("timestamp")
                .and_then(|t| t.as_str())
                .map(ToString::to_string);
        }
        if let Ok(meta) = serde_json::from_value::<SessionMetaLine>(val.clone())
            && let Some(cwd) = meta.meta.cwd.to_str()
        {
            current_cwd = Some(normalize_cwd(cwd));
        }
        if let Ok(rollout) = serde_json::from_value::<RolloutLine>(val.clone())
            && let RolloutItem::TurnContext(tc) = rollout.item
        {
            current_cwd = Some(normalize_cwd(tc.cwd.to_string_lossy().as_ref()));
        }
        let key = current_cwd
            .clone()
            .unwrap_or_else(|| "_unknown".to_string());
        groups.entry(key).or_default().push(val.take());
    }

    if groups.len() <= 1 {
        return Ok(());
    }

    let ts_segment = timestamp_segment_from_filename(path)
        .or(first_ts)
        .unwrap_or_else(|| Utc::now().format("%Y-%m-%dT%H-%M-%S").to_string());

    for (cwd_key, mut items) in groups {
        let new_id = ConversationId::new();
        for val in items.iter_mut() {
            if let Ok(mut meta) = serde_json::from_value::<SessionMetaLine>(val.clone()) {
                meta.meta.id = new_id;
                meta.meta.cwd = PathBuf::from(&cwd_key);
                *val = serde_json::to_value(meta)?;
            }
        }
        let file_name = format!("rollout-{ts_segment}-{new_id}.jsonl");
        let new_path = path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(file_name);
        let mut fh = fs::File::create(&new_path)?;
        for v in items {
            writeln!(fh, "{}", serde_json::to_string(&v)?)?;
        }
    }

    // keep original as backup
    let backup = path.with_extension("mixed.bak");
    let _ = fs::rename(path, backup);
    Ok(())
}

fn migrate_file_if_needed(root: &Path, path: &Path) -> Result<()> {
    let rel = match path.strip_prefix(root) {
        Ok(r) => r,
        Err(_) => return Ok(()),
    };
    let mut comps = rel.components();
    let Some(year) = comps.next() else {
        return Ok(());
    };
    let Some(month) = comps.next() else {
        return Ok(());
    };
    let Some(day) = comps.next() else {
        return Ok(());
    };
    let Some(_file) = comps.next() else {
        return Ok(());
    };
    if comps.next().is_some() {
        // Already under slug or unexpected layout; skip.
        return Ok(());
    }

    let Some(cwd) = read_cwd(path) else {
        return Ok(());
    };
    let slug = slug_for_cwd(&cwd);

    let mut target_dir = root.to_path_buf();
    // comps: [year, month, day, file]
    target_dir.push(year.as_os_str());
    target_dir.push(month.as_os_str());
    target_dir.push(day.as_os_str());
    target_dir.push(slug);
    fs::create_dir_all(&target_dir)?;

    let filename = path
        .file_name()
        .map(std::ffi::OsStr::to_os_string)
        .unwrap_or_default();
    let mut target_path = target_dir.join(&filename);
    if target_path.exists() {
        // Avoid collision: regenerate id
        if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
            let new_id = ConversationId::new();
            let new_name = format!("{stem}-{new_id}.jsonl");
            target_path = target_dir.join(new_name);
        }
    }
    fs::rename(path, target_path)?;
    Ok(())
}

fn normalize_cwd(cwd: &str) -> String {
    cwd.replace('\\', "/")
        .trim_start_matches("//?/")
        .trim_start_matches("\\\\?\\")
        .trim_end_matches('/')
        .to_ascii_lowercase()
}

fn timestamp_segment_from_filename(path: &Path) -> Option<String> {
    let stem = path.file_stem()?.to_string_lossy();
    let rest = stem.strip_prefix("rollout-")?;
    let pos = rest.rfind('-')?;
    Some(rest[..pos].to_string())
}

fn read_cwd(path: &Path) -> Option<std::path::PathBuf> {
    let file = fs::File::open(path).ok()?;
    let reader = BufReader::new(file);
    for line in reader.lines().map_while(Result::ok) {
        if line.trim().is_empty() {
            continue;
        }
        if let Ok(meta) = serde_json::from_str::<SessionMetaLine>(&line) {
            return Some(meta.meta.cwd);
        }
        if let Ok(rollout) = serde_json::from_str::<RolloutLine>(&line) {
            match rollout.item {
                RolloutItem::SessionMeta(session) => return Some(session.meta.cwd),
                RolloutItem::TurnContext(tc) => return Some(tc.cwd),
                _ => {}
            }
        }
    }
    None
}
