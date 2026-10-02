use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use thiserror::Error;

const MAX_VDF_DEPTH: usize = 64;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum VdfError {
    #[error("Unexpected end of input at line {line}, col {col}")]
    UnexpectedEof { line: usize, col: usize },

    #[error("Unterminated string literal at line {line}, col {col}")]
    UnterminatedString { line: usize, col: usize },

    #[error("Expected {expected} at line {line}, col {col}, found '{found}'")]
    SyntaxError {
        expected: String,
        found: String,
        line: usize,
        col: usize,
    },

    #[error("Exceeded maximum recursion depth of {0}")]
    RecursionLimitExceeded(usize),

    #[error("Missing required field '{field}' in manifest")]
    MissingManifestField { field: &'static str },

    #[error("Invalid integer in manifest field '{field}': '{value}'")]
    InvalidManifestNumber {
        field: &'static str,
        value: String,
    },

    #[error("I/O error reading VDF file: {0}")]
    Io(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VdfValue {
    String(String),
    Object(BTreeMap<String, VdfValue>),
}

impl VdfValue {
    pub fn as_str(&self) -> Option<&str> {
        match self {
            VdfValue::String(s) => Some(s.as_str()),
            VdfValue::Object(_) => None,
        }
    }

    pub fn as_object(&self) -> Option<&BTreeMap<String, VdfValue>> {
        match self {
            VdfValue::Object(m) => Some(m),
            VdfValue::String(_) => None,
        }
    }

    pub fn get(&self, key: &str) -> Option<&VdfValue> {
        self.as_object().and_then(|map| {
            // Case-insensitive lookup fallback if direct lookup fails
            map.get(key).or_else(|| {
                map.iter()
                    .find(|(k, _)| k.eq_ignore_ascii_case(key))
                    .map(|(_, v)| v)
            })
        })
    }

    pub fn get_str(&self, key: &str) -> Option<&str> {
        self.get(key).and_then(|v| v.as_str())
    }

    pub fn get_u32(&self, key: &str) -> Option<u32> {
        self.get_str(key).and_then(|s| s.trim().parse::<u32>().ok())
    }

    pub fn get_u64(&self, key: &str) -> Option<u64> {
        self.get_str(key).and_then(|s| s.trim().parse::<u64>().ok())
    }

    pub fn get_bool(&self, key: &str) -> Option<bool> {
        self.get_str(key).map(|s| {
            let trimmed = s.trim();
            trimmed == "1" || trimmed.eq_ignore_ascii_case("true")
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TokenWithPos {
    token: RawToken,
    line: usize,
    col: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum RawToken {
    String(String),
    OpenBrace,
    CloseBrace,
}

fn tokenize(input: &str) -> Result<Vec<TokenWithPos>, VdfError> {
    let mut tokens = Vec::new();
    let mut chars = input.chars().peekable();
    let mut line = 1usize;
    let mut col = 1usize;

    while let Some(&ch) = chars.peek() {
        match ch {
            ' ' | '\t' | '\r' => {
                chars.next();
                col += 1;
            }
            '\n' => {
                chars.next();
                line += 1;
                col = 1;
            }
            '/' => {
                let start_col = col;
                chars.next();
                col += 1;
                if let Some(&next_ch) = chars.peek() {
                    if next_ch == '/' {
                        chars.next();
                        col += 1;
                        // Single-line comment: skip to newline
                        while let Some(&c) = chars.peek() {
                            if c == '\n' {
                                break;
                            }
                            chars.next();
                            col += 1;
                        }
                    } else {
                        // Unquoted string starting with '/'
                        let mut s = String::from("/");
                        while let Some(&c) = chars.peek() {
                            if c.is_whitespace() || c == '{' || c == '}' || c == '"' {
                                break;
                            }
                            s.push(c);
                            chars.next();
                            col += 1;
                        }
                        tokens.push(TokenWithPos {
                            token: RawToken::String(s),
                            line,
                            col: start_col,
                        });
                    }
                } else {
                    tokens.push(TokenWithPos {
                        token: RawToken::String("/".to_string()),
                        line,
                        col: start_col,
                    });
                }
            }
            '{' => {
                tokens.push(TokenWithPos {
                    token: RawToken::OpenBrace,
                    line,
                    col,
                });
                chars.next();
                col += 1;
            }
            '}' => {
                tokens.push(TokenWithPos {
                    token: RawToken::CloseBrace,
                    line,
                    col,
                });
                chars.next();
                col += 1;
            }
            '"' => {
                let start_line = line;
                let start_col = col;
                chars.next(); // Consume opening quote
                col += 1;
                let mut content = String::new();
                let mut closed = false;

                while let Some(&c) = chars.peek() {
                    match c {
                        '"' => {
                            chars.next();
                            col += 1;
                            closed = true;
                            break;
                        }
                        '\\' => {
                            chars.next();
                            col += 1;
                            if let Some(&escaped) = chars.peek() {
                                chars.next();
                                col += 1;
                                match escaped {
                                    '"' => content.push('"'),
                                    '\\' => content.push('\\'),
                                    'n' => content.push('\n'),
                                    't' => content.push('\t'),
                                    'r' => content.push('\r'),
                                    other => {
                                        content.push('\\');
                                        content.push(other);
                                    }
                                }
                            }
                        }
                        '\n' => {
                            content.push(c);
                            chars.next();
                            line += 1;
                            col = 1;
                        }
                        _ => {
                            content.push(c);
                            chars.next();
                            col += 1;
                        }
                    }
                }

                if !closed {
                    return Err(VdfError::UnterminatedString {
                        line: start_line,
                        col: start_col,
                    });
                }

                tokens.push(TokenWithPos {
                    token: RawToken::String(content),
                    line: start_line,
                    col: start_col,
                });
            }
            _ => {
                // Unquoted string token
                let start_line = line;
                let start_col = col;
                let mut s = String::new();
                while let Some(&c) = chars.peek() {
                    if c.is_whitespace() || c == '{' || c == '}' || c == '"' {
                        break;
                    }
                    s.push(c);
                    chars.next();
                    col += 1;
                }
                tokens.push(TokenWithPos {
                    token: RawToken::String(s),
                    line: start_line,
                    col: start_col,
                });
            }
        }
    }

    Ok(tokens)
}

struct Parser<'a> {
    tokens: &'a [TokenWithPos],
    cursor: usize,
}

impl<'a> Parser<'a> {
    fn new(tokens: &'a [TokenWithPos]) -> Self {
        Self { tokens, cursor: 0 }
    }

    fn peek(&self) -> Option<&'a TokenWithPos> {
        self.tokens.get(self.cursor)
    }

    fn next(&mut self) -> Option<&'a TokenWithPos> {
        let tok = self.tokens.get(self.cursor);
        if tok.is_some() {
            self.cursor += 1;
        }
        tok
    }

    fn parse_object(&mut self, depth: usize) -> Result<BTreeMap<String, VdfValue>, VdfError> {
        if depth > MAX_VDF_DEPTH {
            return Err(VdfError::RecursionLimitExceeded(MAX_VDF_DEPTH));
        }

        let mut map = BTreeMap::new();

        while let Some(tok) = self.peek() {
            match &tok.token {
                RawToken::CloseBrace => {
                    break;
                }
                RawToken::String(key_str) => {
                    let key = key_str.clone();
                    self.next(); // consume key

                    let next_tok = self.peek().ok_or_else(|| VdfError::UnexpectedEof {
                        line: tok.line,
                        col: tok.col,
                    })?;

                    match &next_tok.token {
                        RawToken::OpenBrace => {
                            self.next(); // consume '{'
                            let sub_obj = self.parse_object(depth + 1)?;
                            let close_tok = self.next().ok_or_else(|| VdfError::UnexpectedEof {
                                line: next_tok.line,
                                col: next_tok.col,
                            })?;
                            if close_tok.token != RawToken::CloseBrace {
                                return Err(VdfError::SyntaxError {
                                    expected: "}".to_string(),
                                    found: format!("{:?}", close_tok.token),
                                    line: close_tok.line,
                                    col: close_tok.col,
                                });
                            }
                            map.insert(key, VdfValue::Object(sub_obj));
                        }
                        RawToken::String(val_str) => {
                            let val = val_str.clone();
                            self.next(); // consume val
                            map.insert(key, VdfValue::String(val));
                        }
                        RawToken::CloseBrace => {
                            return Err(VdfError::SyntaxError {
                                expected: "value or {".to_string(),
                                found: "}".to_string(),
                                line: next_tok.line,
                                col: next_tok.col,
                            });
                        }
                    }
                }
                RawToken::OpenBrace => {
                    return Err(VdfError::SyntaxError {
                        expected: "string key".to_string(),
                        found: "{".to_string(),
                        line: tok.line,
                        col: tok.col,
                    });
                }
            }
        }

        Ok(map)
    }
}

pub fn parse_vdf(input: &str) -> Result<VdfValue, VdfError> {
    let tokens = tokenize(input)?;
    let mut parser = Parser::new(&tokens);
    let map = parser.parse_object(0)?;
    Ok(VdfValue::Object(map))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SteamAppManifest {
    pub appid: u32,
    pub name: String,
    pub installdir: String,
    pub size_on_disk: u64,
    pub build_id: u64,
    pub bytes_to_download: u64,
    pub bytes_downloaded: u64,
    pub last_updated: u64,
    pub compat_tool: Option<String>,
}

pub fn parse_app_manifest(input: &str) -> Result<SteamAppManifest, VdfError> {
    let root = parse_vdf(input)?;
    let state = root
        .get("AppState")
        .or_else(|| root.get("appstate"))
        .unwrap_or(&root);

    let appid = state
        .get_u32("appid")
        .ok_or(VdfError::MissingManifestField { field: "appid" })?;

    let name = state
        .get_str("name")
        .unwrap_or("Unknown Application")
        .to_string();

    let installdir = state
        .get_str("installdir")
        .ok_or(VdfError::MissingManifestField { field: "installdir" })?
        .to_string();

    let size_on_disk = state.get_u64("SizeOnDisk").unwrap_or(0);
    let build_id = state.get_u64("buildid").unwrap_or(0);
    let bytes_to_download = state.get_u64("BytesToDownload").unwrap_or(0);
    let bytes_downloaded = state.get_u64("BytesDownloaded").unwrap_or(0);
    let last_updated = state.get_u64("LastUpdated").unwrap_or(0);

    let compat_tool = state
        .get("CompatToolMapping")
        .and_then(|mapping| mapping.get("0"))
        .and_then(|tool0| tool0.get_str("name"))
        .map(|s| s.to_string());

    Ok(SteamAppManifest {
        appid,
        name,
        installdir,
        size_on_disk,
        build_id,
        bytes_to_download,
        bytes_downloaded,
        last_updated,
        compat_tool,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SteamLibraryFolder {
    pub path: PathBuf,
    pub label: String,
    pub totalsize: u64,
    pub apps: Vec<u32>,
}

pub fn parse_library_folders(input: &str) -> Result<Vec<SteamLibraryFolder>, VdfError> {
    let root = parse_vdf(input)?;
    let container = root
        .get("libraryfolders")
        .or_else(|| root.get("LibraryFolders"))
        .unwrap_or(&root);

    let mut folders = Vec::new();

    if let Some(obj) = container.as_object() {
        for (_idx_str, val) in obj {
            if let Some(folder_map) = val.as_object() {
                let path_str = folder_map
                    .get("path")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                if path_str.is_empty() {
                    continue;
                }

                let label = folder_map
                    .get("label")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();

                let totalsize = folder_map
                    .get("totalsize")
                    .and_then(|v| v.as_str())
                    .and_then(|s| s.parse::<u64>().ok())
                    .unwrap_or(0);

                let mut apps = Vec::new();
                if let Some(apps_val) = folder_map.get("apps") {
                    if let Some(apps_map) = apps_val.as_object() {
                        for (app_key, _) in apps_map {
                            if let Ok(id) = app_key.parse::<u32>() {
                                apps.push(id);
                            }
                        }
                    }
                }

                folders.push(SteamLibraryFolder {
                    path: PathBuf::from(path_str),
                    label,
                    totalsize,
                    apps,
                });
            }
        }
    }

    Ok(folders)
}

pub fn detect_proton_prefix(library_path: &Path, appid: u32) -> Option<PathBuf> {
    let pfx = library_path
        .join("steamapps")
        .join("compatdata")
        .join(appid.to_string())
        .join("pfx");

    if pfx.exists() {
        Some(pfx)
    } else {
        None
    }
}

pub fn detect_game_install_path(library_path: &Path, installdir: &str) -> PathBuf {
    library_path
        .join("steamapps")
        .join("common")
        .join(installdir)
}

pub fn find_app_in_libraries<'a>(
    libraries: &'a [SteamLibraryFolder],
    appid: u32,
) -> Option<&'a SteamLibraryFolder> {
    libraries.iter().find(|lib| lib.apps.contains(&appid))
}

#[derive(Debug, Clone)]
pub struct AppResolution {
    pub manifest: SteamAppManifest,
    pub library_path: PathBuf,
    pub install_path: PathBuf,
    pub proton_prefix: Option<PathBuf>,
}

pub fn locate_app_in_library(
    library_path: &Path,
    appid: u32,
) -> Result<AppResolution, VdfError> {
    let manifest_file = library_path
        .join("steamapps")
        .join(format!("appmanifest_{}.acf", appid));

    if !manifest_file.exists() {
        return Err(VdfError::Io(format!(
            "App manifest not found: {}",
            manifest_file.display()
        )));
    }

    let content = std::fs::read_to_string(&manifest_file)
        .map_err(|e| VdfError::Io(format!("Failed to read {}: {}", manifest_file.display(), e)))?;

    let manifest = parse_app_manifest(&content)?;
    let install_path = detect_game_install_path(library_path, &manifest.installdir);
    let proton_prefix = detect_proton_prefix(library_path, appid);

    Ok(AppResolution {
        manifest,
        library_path: library_path.to_path_buf(),
        install_path,
        proton_prefix,
    })
}

pub fn find_standard_steam_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Ok(home) = std::env::var("HOME") {
        let home_path = PathBuf::from(home);
        let standard_candidates = [
            home_path.join(".local/share/Steam"),
            home_path.join(".steam/steam"),
            home_path.join(".steam/root"),
            home_path.join(".var/app/com.valvesoftware.Steam/.local/share/Steam"),
        ];

        for cand in standard_candidates {
            if cand.exists() && !paths.contains(&cand) {
                paths.push(cand);
            }
        }
    }
    paths
}
