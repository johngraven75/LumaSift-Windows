use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::thread;
use tauri::{AppHandle, Manager};
use tiny_http::{Header, Method, Response, Server, StatusCode};
use uuid::Uuid;
use walkdir::WalkDir;

const SAMPLE_BYTES: usize = 1_048_576;
const PURGE_CONFIRMATION: &str = "ERASE LUMASIFT QUARANTINE";
static CANCEL_REQUESTED: AtomicBool = AtomicBool::new(false);
static RUNTIME: OnceLock<Mutex<Runtime>> = OnceLock::new();
static OWNER_SOURCE_SCOPE: OnceLock<Mutex<Vec<String>>> = OnceLock::new();

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum SelectionType {
    Video,
    Audio,
    Document,
    Image,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanRequest {
    pub sources: Vec<String>,
    pub selected_types: Vec<SelectionType>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NasConnectRequest {
    pub unc_path: String,
    pub username: String,
    pub password: String,
    pub remember_connection: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanProgress {
    pub scanning: bool,
    pub phase: String,
    pub current: u64,
    pub total: u64,
    pub percentage: u8,
    pub current_path: Option<String>,
    pub files_considered: u64,
    pub message: String,
    pub error: Option<String>,
}

impl Default for ScanProgress {
    fn default() -> Self {
        Self {
            scanning: false,
            phase: "Ready".to_string(),
            current: 0,
            total: 0,
            percentage: 0,
            current_path: None,
            files_considered: 0,
            message: "Choose sources and file types. LumaSift will build a review-only plan before moving anything.".to_string(),
            error: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct QualityEvidence {
    pub width: Option<u64>,
    pub height: Option<u64>,
    pub pixel_count: u64,
    pub bitrate: Option<u64>,
    pub bit_depth: Option<u64>,
    pub duration_millis: Option<u64>,
    pub file_size_bytes: u64,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Candidate {
    pub id: String,
    pub file_path: String,
    pub display_name: String,
    pub media_kind: String,
    pub exact_hash: String,
    pub quality_score: u64,
    pub quality: QualityEvidence,
    pub disposition: String,
    pub disposition_detail: String,
    pub quarantine_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateGroup {
    pub id: String,
    pub exact_hash: String,
    pub winner_id: String,
    pub reclaimable_bytes: u64,
    pub candidates: Vec<Candidate>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Disposition {
    pub occurred_at: String,
    pub file_path: String,
    pub display_name: String,
    pub disposition: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolutionPlan {
    pub id: String,
    pub status: String,
    pub selected_types: Vec<SelectionType>,
    pub created_at: String,
    pub groups: Vec<DuplicateGroup>,
    pub reclaimable_bytes: u64,
    pub queued_file_count: u64,
    pub dispositions: Vec<Disposition>,
}

#[derive(Debug, Clone)]
struct IndexedFile {
    path: PathBuf,
    media_kind: String,
    bytes: u64,
}

#[derive(Default)]
struct Runtime {
    progress: ScanProgress,
    plan: Option<ResolutionPlan>,
}

fn runtime() -> &'static Mutex<Runtime> {
    RUNTIME.get_or_init(|| Mutex::new(Runtime::default()))
}

fn state() -> std::sync::MutexGuard<'static, Runtime> {
    runtime().lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn owner_source_scope() -> &'static Mutex<Vec<String>> {
    OWNER_SOURCE_SCOPE.get_or_init(|| Mutex::new(Vec::new()))
}

fn display_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string_lossy().into_owned())
}

fn update_progress(phase: &str, current: u64, total: u64, percentage: u8, path: Option<String>, message: &str) {
    let mut runtime = state();
    runtime.progress = ScanProgress {
        scanning: true,
        phase: phase.to_string(),
        current,
        total,
        percentage: percentage.min(99),
        current_path: path,
        files_considered: runtime.progress.files_considered,
        message: message.to_string(),
        error: None,
    };
}

fn fail_progress(message: String) {
    let mut runtime = state();
    runtime.progress.scanning = false;
    runtime.progress.phase = "Failed".to_string();
    runtime.progress.current_path = None;
    runtime.progress.error = Some(message.clone());
    runtime.progress.message = "LumaSift stopped before changing any files.".to_string();
}

fn classify(path: &Path, selected: &HashSet<SelectionType>) -> Option<String> {
    let extension = path.extension()?.to_string_lossy().to_ascii_lowercase();
    let kind = match extension.as_str() {
        "mp4" | "mkv" | "mov" | "avi" | "wmv" | "webm" | "m4v" if selected.contains(&SelectionType::Video) => "video",
        "mp3" if selected.contains(&SelectionType::Audio) => "audio",
        "docx" | "pdf" if selected.contains(&SelectionType::Document) => "document",
        "jpg" | "jpeg" | "png" | "gif" | "webp" | "bmp" | "tif" | "tiff" | "heic" if selected.contains(&SelectionType::Image) => "image",
        _ => return None,
    };
    Some(kind.to_string())
}

fn validate_request(request: &ScanRequest) -> Result<(), String> {
    if request.sources.is_empty() {
        return Err("Select at least one local, external, or NAS source before scanning.".to_string());
    }
    if request.selected_types.is_empty() {
        return Err("Select at least one file category before scanning.".to_string());
    }
    if request.sources.iter().any(|source| source.trim().is_empty()) {
        return Err("A source path may not be blank.".to_string());
    }
    Ok(())
}

fn source_files(request: &ScanRequest, app_data: &Path) -> Result<Vec<IndexedFile>, String> {
    let selected = request.selected_types.iter().cloned().collect::<HashSet<_>>();
    let app_data = app_data.canonicalize().unwrap_or_else(|_| app_data.to_path_buf());
    let mut seen = HashSet::new();
    let mut files = Vec::new();
    let mut visited_entries = 0_u64;

    for source in &request.sources {
        let source_path = PathBuf::from(source);
        if !source_path.is_dir() {
            return Err(format!("LumaSift source is not an accessible directory: {source}"));
        }
        for entry in WalkDir::new(&source_path).follow_links(false).into_iter().filter_map(Result::ok) {
            visited_entries += 1;
            if visited_entries % 32 == 0 {
                update_progress("Indexing sources", visited_entries, 0, 1, Some(entry.path().to_string_lossy().into_owned()), "Walking selected sources in the background. No files will be changed.");
            }
            if CANCEL_REQUESTED.load(Ordering::Relaxed) { return Err("LumaSift scan cancelled while indexing sources.".to_string()); }
            if !entry.file_type().is_file() {
                continue;
            }
            let path = entry.path();
            let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
            if canonical.starts_with(&app_data) || !seen.insert(canonical.clone()) {
                continue;
            }
            let Some(media_kind) = classify(&canonical, &selected) else { continue; };
            let bytes = match canonical.metadata() {
                Ok(metadata) => metadata.len(),
                Err(_) => continue,
            };
            files.push(IndexedFile { path: canonical, media_kind, bytes });
        }
    }
    files.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(files)
}

fn sample_hash(path: &Path, bytes: u64) -> Result<String, String> {
    let mut file = File::open(path).map_err(|error| format!("Unable to open {}: {error}", path.display()))?;
    let mut hasher = Sha256::new();
    hasher.update(bytes.to_le_bytes());
    let mut head = vec![0_u8; SAMPLE_BYTES.min(bytes as usize)];
    let head_read = file.read(&mut head).map_err(|error| format!("Unable to read {}: {error}", path.display()))?;
    hasher.update(&head[..head_read]);
    if bytes > SAMPLE_BYTES as u64 {
        file.seek(SeekFrom::Start(bytes.saturating_sub(SAMPLE_BYTES as u64)))
            .map_err(|error| format!("Unable to seek {}: {error}", path.display()))?;
        let mut tail = vec![0_u8; SAMPLE_BYTES];
        let tail_read = file.read(&mut tail).map_err(|error| format!("Unable to read {}: {error}", path.display()))?;
        hasher.update(&tail[..tail_read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn full_hash(path: &Path) -> Result<String, String> {
    let mut file = File::open(path).map_err(|error| format!("Unable to open {}: {error}", path.display()))?;
    let mut buffer = [0_u8; SAMPLE_BYTES];
    let mut hasher = Sha256::new();
    loop {
        let read = file.read(&mut buffer).map_err(|error| format!("Unable to read {}: {error}", path.display()))?;
        if read == 0 { break; }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn evidence(file: &IndexedFile) -> QualityEvidence {
    QualityEvidence {
        file_size_bytes: file.bytes,
        reasons: vec![
            format!("{} exact-content bytes", file.bytes),
            "Full SHA-256 proof is the action gate; equal content ties are resolved deterministically by path.".to_string(),
        ],
        ..Default::default()
    }
}

fn append_disposition(dispositions: &mut Vec<Disposition>, path: &Path, disposition: &str, detail: impl Into<String>) {
    dispositions.push(Disposition {
        occurred_at: Utc::now().to_rfc3339(),
        file_path: path.to_string_lossy().into_owned(),
        display_name: display_name(path),
        disposition: disposition.to_string(),
        detail: detail.into(),
    });
}

fn cancel_plan(dispositions: Vec<Disposition>, selected_types: Vec<SelectionType>) {
    let mut runtime = state();
    runtime.progress.scanning = false;
    runtime.progress.phase = "Cancelled".to_string();
    runtime.progress.current_path = None;
    runtime.progress.message = "LumaSift cancelled before producing an actionable plan. No files changed.".to_string();
    runtime.plan = Some(ResolutionPlan {
        id: Uuid::new_v4().to_string(),
        status: "cancelled".to_string(),
        selected_types,
        created_at: Utc::now().to_rfc3339(),
        groups: Vec::new(),
        reclaimable_bytes: 0,
        queued_file_count: 0,
        dispositions,
    });
}

fn persist_plan(app_data: &Path, plan: &ResolutionPlan) {
    let directory = app_data.join("lumasift");
    if fs::create_dir_all(&directory).is_ok() {
        if let Ok(serialized) = serde_json::to_vec_pretty(plan) {
            let _ = fs::write(directory.join("last-resolution-plan.json"), serialized);
        }
    }
}

fn build_plan(files: Vec<IndexedFile>, selected_types: Vec<SelectionType>, app_data: PathBuf) {
    let mut dispositions = Vec::new();
    let total = files.len() as u64;
    let mut sampled: HashMap<(u64, String), Vec<IndexedFile>> = HashMap::new();
    for (index, file) in files.into_iter().enumerate() {
        if CANCEL_REQUESTED.load(Ordering::Relaxed) {
            cancel_plan(dispositions, selected_types);
            return;
        }
        update_progress("Sampling content", index as u64 + 1, total, if total == 0 { 60 } else { ((index as u64 + 1) * 60 / total) as u8 }, Some(file.path.to_string_lossy().into_owned()), "Building collision candidates without changing any files.");
        match sample_hash(&file.path, file.bytes) {
            Ok(hash) => sampled.entry((file.bytes, hash)).or_default().push(file),
            Err(error) => append_disposition(&mut dispositions, &file.path, "skipped", error),
        }
    }

    let candidate_groups = sampled.into_values().filter(|group| group.len() > 1).collect::<Vec<_>>();
    let verification_total = candidate_groups.iter().map(Vec::len).sum::<usize>() as u64;
    let mut verified: HashMap<String, Vec<IndexedFile>> = HashMap::new();
    let mut verified_current = 0_u64;
    for group in candidate_groups {
        for file in group {
            if CANCEL_REQUESTED.load(Ordering::Relaxed) {
                cancel_plan(dispositions, selected_types);
                return;
            }
            verified_current += 1;
            let percentage = if verification_total == 0 { 90 } else { 60 + ((verified_current * 30 / verification_total) as u8) };
            update_progress("Verifying exact matches", verified_current, verification_total, percentage, Some(file.path.to_string_lossy().into_owned()), "Calculating full SHA-256 digests before a file may enter a resolution plan.");
            match full_hash(&file.path) {
                Ok(hash) => verified.entry(hash).or_default().push(file),
                Err(error) => append_disposition(&mut dispositions, &file.path, "skipped", error),
            }
        }
    }

    let exact_groups = verified.into_iter().filter(|(_, group)| group.len() > 1).collect::<Vec<_>>();
    let exact_total = exact_groups.len() as u64;
    let mut groups = Vec::new();
    let mut reclaimable_bytes = 0_u64;
    let mut queued_file_count = 0_u64;
    for (index, (exact_hash, mut files)) in exact_groups.into_iter().enumerate() {
        if CANCEL_REQUESTED.load(Ordering::Relaxed) {
            cancel_plan(dispositions, selected_types);
            return;
        }
        let percentage = if exact_total == 0 { 99 } else { 90 + ((index as u64 + 1) * 9 / exact_total) as u8 };
        update_progress("Ranking retained copies", index as u64 + 1, exact_total, percentage, None, "Exact content has equal source quality. Applying deterministic evidence and a stable path tie-break.");
        files.sort_by(|left, right| right.bytes.cmp(&left.bytes).then_with(|| left.path.cmp(&right.path)));
        let mut candidates = files.into_iter().map(|file| {
            let quality = evidence(&file);
            Candidate {
                id: Uuid::new_v4().to_string(),
                file_path: file.path.to_string_lossy().into_owned(),
                display_name: display_name(&file.path),
                media_kind: file.media_kind,
                exact_hash: exact_hash.clone(),
                quality_score: quality.file_size_bytes,
                quality,
                disposition: "pending_review".to_string(),
                disposition_detail: "Awaiting an owner-reviewed resolution plan.".to_string(),
                quarantine_path: None,
            }
        }).collect::<Vec<_>>();
        let winner_id = candidates.first().expect("exact group has candidates").id.clone();
        let mut group_reclaimable = 0_u64;
        for candidate in &mut candidates {
            if candidate.id == winner_id {
                candidate.disposition = "retain".to_string();
                candidate.disposition_detail = "Exact-content tie resolved deterministically; this copy remains in place.".to_string();
                append_disposition(&mut dispositions, Path::new(&candidate.file_path), "retain", candidate.disposition_detail.clone());
            } else {
                candidate.disposition = "queued_for_quarantine".to_string();
                candidate.disposition_detail = "Lower-ranked exact duplicate. It will move only after your approval.".to_string();
                group_reclaimable = group_reclaimable.saturating_add(candidate.quality.file_size_bytes);
                queued_file_count += 1;
                append_disposition(&mut dispositions, Path::new(&candidate.file_path), "queued_for_quarantine", candidate.disposition_detail.clone());
            }
        }
        reclaimable_bytes = reclaimable_bytes.saturating_add(group_reclaimable);
        groups.push(DuplicateGroup { id: Uuid::new_v4().to_string(), exact_hash, winner_id, reclaimable_bytes: group_reclaimable, candidates });
    }

    let plan = ResolutionPlan {
        id: Uuid::new_v4().to_string(),
        status: "ready_for_review".to_string(),
        selected_types,
        created_at: Utc::now().to_rfc3339(),
        groups,
        reclaimable_bytes,
        queued_file_count,
        dispositions,
    };
    persist_plan(&app_data, &plan);
    let mut runtime = state();
    runtime.progress = ScanProgress {
        scanning: false,
        phase: "Review ready".to_string(),
        current: total,
        total,
        percentage: 100,
        current_path: None,
        files_considered: total,
        message: format!("{} exact duplicate groups are ready for review. {} files are queued for quarantine.", plan.groups.len(), plan.queued_file_count),
        error: None,
    };
    runtime.plan = Some(plan);
}

fn quarantine_root(app: &AppHandle) -> Result<PathBuf, String> {
    app.path().app_data_dir()
        .map(|directory| directory.join("lumasift").join("quarantine"))
        .map_err(|error| format!("Unable to access LumaSift app data: {error}"))
}

fn unique_destination(directory: &Path, source: &Path) -> Result<PathBuf, String> {
    let file_name = source.file_name().ok_or_else(|| format!("Source path has no name: {}", source.display()))?;
    let direct = directory.join(file_name);
    if !direct.exists() { return Ok(direct); }
    let stem = source.file_stem().and_then(|value| value.to_str()).unwrap_or("duplicate");
    let extension = source.extension().and_then(|value| value.to_str());
    for suffix in 1..=10_000_u32 {
        let candidate = match extension { Some(extension) if !extension.is_empty() => directory.join(format!("{stem} ({suffix}).{extension}")), _ => directory.join(format!("{stem} ({suffix})")) };
        if !candidate.exists() { return Ok(candidate); }
    }
    Err("Unable to allocate a unique quarantine destination.".to_string())
}

fn move_without_overwrite(source: &Path, destination: &Path) -> Result<(), String> {
    if destination.exists() { return Err("LumaSift refused to overwrite an existing quarantine file.".to_string()); }
    match fs::rename(source, destination) {
        Ok(()) => Ok(()),
        Err(rename_error) => {
            fs::copy(source, destination).map_err(|copy_error| format!("Could not move {} ({rename_error}); copy fallback failed: {copy_error}", source.display()))?;
            if let Err(remove_error) = fs::remove_file(source) {
                let _ = fs::remove_file(destination);
                return Err(format!("Copied {} but could not remove its original: {remove_error}", source.display()));
            }
            Ok(())
        }
    }
}

#[cfg(target_os = "windows")]
fn wide(value: &str) -> Vec<u16> { value.encode_utf16().chain(std::iter::once(0)).collect() }

#[cfg(target_os = "windows")]
fn connect_nas(request: &NasConnectRequest) -> Result<(), String> {
    use windows::core::{PCWSTR, PWSTR};
    use windows::Win32::NetworkManagement::WNet::{WNetAddConnection2W, CONNECT_UPDATE_PROFILE, NET_CONNECT_FLAGS, NETRESOURCEW, RESOURCETYPE_DISK};
    if !request.unc_path.starts_with(r"\\") { return Err("NAS paths must use a UNC path such as \\server\\share.".to_string()); }
    let mut remote = wide(&request.unc_path);
    let username = wide(&request.username);
    let password = wide(&request.password);
    let resource = NETRESOURCEW { dwType: RESOURCETYPE_DISK, lpRemoteName: PWSTR(remote.as_mut_ptr()), ..Default::default() };
    let flags = if request.remember_connection { CONNECT_UPDATE_PROFILE } else { NET_CONNECT_FLAGS(0) };
    let status = unsafe { WNetAddConnection2W(&resource, PCWSTR(password.as_ptr()), PCWSTR(username.as_ptr()), flags) };
    if status.0 != 0 { return Err(format!("Windows could not connect the NAS source (error {}).", status.0)); }
    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn connect_nas(_: &NasConnectRequest) -> Result<(), String> { Err("Native NAS connection is available only in the Windows build.".to_string()) }

#[tauri::command]
fn get_scan_progress() -> ScanProgress { state().progress.clone() }

#[tauri::command]
fn get_resolution_plan() -> Option<ResolutionPlan> { state().plan.clone() }

#[tauri::command]
fn start_resolution(app: AppHandle, request: ScanRequest) -> Result<ScanProgress, String> {
    validate_request(&request)?;
    if state().progress.scanning { return Err("A LumaSift scan is already in progress.".to_string()); }
    let app_data = app.path().app_data_dir().map_err(|error| format!("Unable to access LumaSift app data: {error}"))?;
    start_with_app_data(app_data, request)
}

#[tauri::command]
fn cancel_resolution() -> Result<(), String> {
    if !state().progress.scanning { return Err("No LumaSift scan is active.".to_string()); }
    CANCEL_REQUESTED.store(true, Ordering::Relaxed);
    Ok(())
}

#[tauri::command]
fn connect_nas_source(request: NasConnectRequest) -> Result<(), String> {
    if request.username.trim().is_empty() || request.password.is_empty() { return Err("NAS credentials are required to connect this source.".to_string()); }
    connect_nas(&request)
}

#[tauri::command]
fn apply_resolution_plan(app: AppHandle, plan_id: String) -> Result<serde_json::Value, String> {
    apply_with_root(quarantine_root(&app)?, plan_id)
}

#[tauri::command]
fn purge_quarantine(app: AppHandle, confirmation: String) -> Result<serde_json::Value, String> {
    if confirmation.trim() != PURGE_CONFIRMATION { return Err(format!("Permanent erase requires the exact confirmation: {PURGE_CONFIRMATION}")); }
    let root = quarantine_root(&app)?;
    if !root.exists() { return Ok(serde_json::json!({ "status": "empty", "erased": 0 })); }
    let mut erased = 0_u64;
    for entry in WalkDir::new(&root).follow_links(false).contents_first(true).into_iter().filter_map(Result::ok) {
        let path = entry.path();
        if entry.file_type().is_file() { fs::remove_file(path).map_err(|error| format!("Unable to permanently erase {}: {error}", path.display()))?; erased += 1; }
        else if path != root { let _ = fs::remove_dir(path); }
    }
    let _ = fs::remove_dir(&root);
    Ok(serde_json::json!({ "status": "erased", "erased": erased, "message": "LumaSift quarantine was permanently erased after explicit confirmation." }))
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct CompanionAccess {
    local_url: String,
    access_token: String,
    tls_notice: String,
}

#[derive(Debug, Deserialize)]
struct CompanionStartRequest {
    selected_types: Vec<SelectionType>,
}

#[derive(Debug, Deserialize)]
struct CompanionApplyRequest {
    plan_id: String,
}

static COMPANION_ACCESS: OnceLock<CompanionAccess> = OnceLock::new();

fn companion_access() -> Result<CompanionAccess, String> {
    COMPANION_ACCESS.get().cloned().ok_or_else(|| "LumaSift companion service is not configured.".to_string())
}

fn companion_token(app_data: &Path) -> Result<String, String> {
    let directory = app_data.join("lumasift");
    fs::create_dir_all(&directory).map_err(|error| format!("Unable to create companion storage: {error}"))?;
    let path = directory.join("companion-access-token.txt");
    if let Ok(token) = fs::read_to_string(&path) {
        let token = token.trim().to_string();
        if !token.is_empty() { return Ok(token); }
    }
    let token = Uuid::new_v4().to_string();
    fs::write(&path, &token).map_err(|error| format!("Unable to persist companion access token: {error}"))?;
    Ok(token)
}

fn snake_case_key(key: &str) -> String {
    key.chars().enumerate().fold(String::new(), |mut output, (index, character)| {
        if character.is_ascii_uppercase() { if index > 0 { output.push('_'); } output.push(character.to_ascii_lowercase()); } else { output.push(character); }
        output
    })
}

fn redact_companion_value(value: serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(map) => serde_json::Value::Object(map.into_iter().filter_map(|(key, value)| {
            if matches!(key.as_str(), "filePath" | "quarantinePath" | "currentPath") { None } else { Some((snake_case_key(&key), redact_companion_value(value))) }
        }).collect()),
        serde_json::Value::Array(items) => serde_json::Value::Array(items.into_iter().map(redact_companion_value).collect()),
        other => other,
    }
}

fn companion_json(status: u16, value: serde_json::Value) -> Response<std::io::Cursor<Vec<u8>>> {
    let body = serde_json::to_vec(&redact_companion_value(value)).unwrap_or_else(|_| b"{\"error\":\"serialization_failed\"}".to_vec());
    Response::from_data(body).with_status_code(StatusCode(status)).with_header(Header::from_bytes("Content-Type", "application/json").expect("static header is valid"))
}

fn start_with_app_data(app_data: PathBuf, request: ScanRequest) -> Result<ScanProgress, String> {
    validate_request(&request)?;
    *owner_source_scope().lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = request.sources.clone();
    if state().progress.scanning { return Err("A LumaSift scan is already in progress.".to_string()); }
    CANCEL_REQUESTED.store(false, Ordering::Relaxed);
    let selected_types = request.selected_types.clone();
    let sources = request.sources.clone();
    let initial = ScanProgress {
        scanning: true,
        phase: "Indexing sources".to_string(),
        current: 0,
        total: 0,
        percentage: 1,
        current_path: None,
        files_considered: 0,
        message: "Enumerating selected sources in the background. The workspace remains responsive.".to_string(),
        error: None,
    };
    { let mut runtime = state(); runtime.progress = initial.clone(); runtime.plan = None; }
    let worker = thread::Builder::new().name("lumasift-resolution".to_string()).spawn(move || {
        let request = ScanRequest { sources, selected_types: selected_types.clone() };
        match source_files(&request, &app_data) {
            Ok(files) => {
                let total = files.len() as u64;
                update_progress("Indexing complete", 0, total, 2, None, &format!("Indexed {total} eligible files. Beginning sampled content proof."));
                build_plan(files, selected_types, app_data);
            }
            Err(error) => fail_progress(error),
        }
    });
    if let Err(error) = worker {
        let message = format!("Unable to start LumaSift worker: {error}");
        fail_progress(message.clone());
        return Err(message);
    }
    Ok(initial)
}

fn apply_with_root(root: PathBuf, plan_id: String) -> Result<serde_json::Value, String> {
    let mut plan = state().plan.clone().ok_or("No LumaSift plan is ready for review.")?;
    if plan.id != plan_id || plan.status != "ready_for_review" { return Err("This LumaSift plan is stale or no longer eligible for application.".to_string()); }
    let directory = root.join(&plan.id);
    fs::create_dir_all(&directory).map_err(|error| format!("Unable to create quarantine: {error}"))?;
    let mut quarantined = 0_u64; let mut failed = 0_u64;
    for group in &mut plan.groups { for candidate in &mut group.candidates {
        if candidate.disposition != "queued_for_quarantine" { continue; }
        let source = PathBuf::from(&candidate.file_path);
        let result = (|| -> Result<PathBuf, String> { if full_hash(&source)? != candidate.exact_hash { return Err("Content changed since review; the file was left in place.".to_string()); } let destination = unique_destination(&directory, &source)?; move_without_overwrite(&source, &destination)?; Ok(destination) })();
        match result { Ok(destination) => { candidate.disposition = "quarantined".to_string(); candidate.disposition_detail = "Moved to LumaSift quarantine. It has not been permanently deleted.".to_string(); candidate.quarantine_path = Some(destination.to_string_lossy().into_owned()); append_disposition(&mut plan.dispositions, &source, "quarantined", candidate.disposition_detail.clone()); quarantined += 1; }, Err(error) => { candidate.disposition = "failed".to_string(); candidate.disposition_detail = error.clone(); append_disposition(&mut plan.dispositions, &source, "failed", error); failed += 1; } }
    }}
    plan.status = if failed == 0 { "applied_to_quarantine".to_string() } else { "partially_applied".to_string() };
    plan.queued_file_count = plan.groups.iter().flat_map(|group| group.candidates.iter()).filter(|candidate| candidate.disposition == "queued_for_quarantine").count() as u64;
    let mut runtime = state(); runtime.progress.message = format!("LumaSift quarantined {quarantined} file(s); {failed} file(s) remained in place."); runtime.plan = Some(plan.clone());
    Ok(serde_json::json!({ "status": plan.status, "plan": plan }))
}

fn serve_companion(app_data: PathBuf, access: CompanionAccess) {
    let Ok(server) = Server::http("127.0.0.1:7417") else { return; };
    for mut request in server.incoming_requests() {
        let authorized = request.headers().iter().any(|header| header.field.equiv("Authorization") && header.value.as_str() == format!("Bearer {}", access.access_token));
        if !authorized { let _ = request.respond(companion_json(401, serde_json::json!({"error":"unauthorized"}))); continue; }
        let path = request.url().to_string();
        let response = match (request.method(), path.as_str()) {
            (&Method::Get, "/api/lumasift/status") => companion_json(200, serde_json::to_value(get_scan_progress()).unwrap_or_default()),
            (&Method::Get, "/api/lumasift/plan") => match get_resolution_plan() { Some(plan) => companion_json(200, serde_json::to_value(plan).unwrap_or_default()), None => companion_json(404, serde_json::json!({"error":"no_plan"})) },
            (&Method::Post, "/api/lumasift/start") => { let mut body = String::new(); let _ = request.as_reader().read_to_string(&mut body); match serde_json::from_str::<CompanionStartRequest>(&body).map_err(|error| error.to_string()).and_then(|payload| start_with_app_data(app_data.clone(), ScanRequest { sources: configured_sources(), selected_types: payload.selected_types })) { Ok(progress) => companion_json(200, serde_json::to_value(progress).unwrap_or_default()), Err(error) => companion_json(400, serde_json::json!({"error":error})) } },
            (&Method::Post, "/api/lumasift/plan/apply") => { let mut body = String::new(); let _ = request.as_reader().read_to_string(&mut body); match serde_json::from_str::<CompanionApplyRequest>(&body).map_err(|error| error.to_string()).and_then(|payload| apply_with_root(app_data.join("lumasift").join("quarantine"), payload.plan_id)) { Ok(result) => companion_json(200, result.get("plan").cloned().unwrap_or(result)), Err(error) => companion_json(400, serde_json::json!({"error":error})) } },
            _ => companion_json(404, serde_json::json!({"error":"not_found"})),
        };
        let _ = request.respond(response);
    }
}

fn configured_sources() -> Vec<String> { owner_source_scope().lock().unwrap_or_else(|poisoned| poisoned.into_inner()).clone() }

#[tauri::command]
fn get_companion_access() -> Result<CompanionAccess, String> { companion_access() }

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| { let app_data = app.path().app_data_dir()?; let token = companion_token(&app_data).map_err(std::io::Error::other)?; let access = CompanionAccess { local_url: "http://127.0.0.1:7417".to_string(), access_token: token, tls_notice: "Expose this local service to mobile companions only through an owner-managed HTTPS reverse proxy on the same Windows host.".to_string() }; let _ = COMPANION_ACCESS.set(access.clone()); thread::spawn(move || serve_companion(app_data, access)); Ok(()) })
        .invoke_handler(tauri::generate_handler![get_scan_progress, get_resolution_plan, get_companion_access, start_resolution, cancel_resolution, connect_nas_source, apply_resolution_plan, purge_quarantine])
        .run(tauri::generate_context!())
        .expect("error while running LumaSift");
}

#[cfg(test)]
mod tests {
    use super::{classify, full_hash, sample_hash, SelectionType};
    use std::collections::HashSet;
    use std::fs;

    #[test]
    fn selected_category_contract_rejects_unselected_extensions() {
        let selected = HashSet::from([SelectionType::Audio]);
        assert_eq!(classify(std::path::Path::new("library/song.mp3"), &selected).as_deref(), Some("audio"));
        assert_eq!(classify(std::path::Path::new("library/movie.mp4"), &selected), None);
    }

    #[test]
    fn full_hash_proves_equal_content_after_sampling() {
        let root = std::env::temp_dir().join(format!("lumasift-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).expect("create test directory");
        let first = root.join("first.pdf");
        let second = root.join("second.pdf");
        fs::write(&first, b"lumasift exact content").expect("write first");
        fs::write(&second, b"lumasift exact content").expect("write second");
        let size = fs::metadata(&first).expect("metadata").len();
        assert_eq!(sample_hash(&first, size).expect("sample first"), sample_hash(&second, size).expect("sample second"));
        assert_eq!(full_hash(&first).expect("hash first"), full_hash(&second).expect("hash second"));
        fs::remove_dir_all(root).expect("remove test directory");
    }
}
