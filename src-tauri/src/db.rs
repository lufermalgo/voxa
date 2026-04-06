use rusqlite::{params, Connection, Result};
use std::fs;
use std::collections::HashMap;
use tauri::AppHandle;
use tauri::Manager;

pub struct DbState {
    pub conn: std::sync::Mutex<Connection>,
}

pub fn init(app_handle: &AppHandle) -> Result<Connection, String> {
    // Get app data directory
    let app_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?;

    // Create directory if it doesn't exist
    if !app_dir.exists() {
        fs::create_dir_all(&app_dir).map_err(|e| e.to_string())?;
    }

    let db_path = app_dir.join("voxa.db");
    
    let conn = Connection::open(db_path).map_err(|e| e.to_string())?;

    // Initialize tables
    init_tables(&conn).map_err(|e| e.to_string())?;

    Ok(conn)
}

fn init_tables(conn: &Connection) -> Result<()> {
    // Transcripts table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS transcripts (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            content TEXT NOT NULL,
            raw_content TEXT NOT NULL,
            timestamp DATETIME DEFAULT CURRENT_TIMESTAMP,
            is_favorite INTEGER DEFAULT 0
        )",
        [],
    )?;

    // Custom dictionary table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS custom_dict (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            word TEXT NOT NULL UNIQUE,
            replacement TEXT,
            category TEXT
        )",
        [],
    )?;
    
    // Settings table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS app_settings (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        )",
        [],
    )?;

    // Transformation profiles table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS transformation_profiles (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL UNIQUE,
            system_prompt TEXT NOT NULL,
            icon TEXT,
            is_default INTEGER DEFAULT 0
        )",
        [],
    )?;

    // Migration: add icon column if missing
    let _ = conn.execute("ALTER TABLE transformation_profiles ADD COLUMN icon TEXT", []);

    // Insert defaults if empty
    conn.execute(
        "INSERT OR IGNORE INTO app_settings (key, value) VALUES 
        ('mic_id', 'none'),
        ('language', 'es'),
        ('interaction_mode', 'push_to_talk'),
        ('global_shortcut', 'Alt+Space'),
        ('active_profile_id', '1'),
        ('is_onboarded', 'false')",
        [],
    )?;

    conn.execute(
        "INSERT OR IGNORE INTO transformation_profiles (id, name, system_prompt, icon, is_default) VALUES 
        (1, 'Elegante', 'Actuá como un asistente profesional. Corregí la gramática, ortografía y puntuación del texto. Devolvé ÚNICAMENTE el texto corregido y bien formateado.', 'star', 1),
        (2, 'Informal', 'Actuá como un compañero de trabajo en un chat informal. Corregí el texto pero mantené un tono relajado y directo. Devolvé ÚNICAMENTE el texto final.', 'forum', 1),
        (3, 'Code', 'Actuá como un experto en programación. Limpiá y formateá el código transcripto, corrigiendo nombres de variables o sintaxis si es obvio, pero manteniendo la lógica. Respondé ÚNICAMENTE con el bloque de código.', 'code', 1),
        (4, 'Custom', 'Instrucciones personalizadas: escribí acá cómo querés que el LLM procese tu texto.', 'tune', 1)",
        [],
    )?;

    // Forced update for existing installations to match requested names
    let _ = conn.execute("UPDATE transformation_profiles SET name = 'Elegante' WHERE id = 1", []);
    let _ = conn.execute("UPDATE transformation_profiles SET name = 'Informal' WHERE id = 2", []);
    let _ = conn.execute("UPDATE transformation_profiles SET name = 'Code' WHERE id = 3", []);
    let _ = conn.execute("UPDATE transformation_profiles SET name = 'Custom' WHERE id = 4", []);

    Ok(())
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct Transcript {
    pub id: i64,
    pub content: String,
    pub raw_content: String,
    pub timestamp: String,
    pub is_favorite: bool,
}

pub fn get_all_transcripts(conn: &Connection) -> Result<Vec<Transcript>> {
    let mut stmt = conn.prepare(
        "SELECT id, content, raw_content, timestamp, is_favorite FROM transcripts ORDER BY timestamp DESC"
    )?;
    
    let transcript_iter = stmt.query_map([], |row| {
        Ok(Transcript {
            id: row.get(0)?,
            content: row.get(1)?,
            raw_content: row.get(2)?,
            timestamp: row.get(3)?,
            is_favorite: row.get::<_, i32>(4)? != 0,
        })
    })?;

    let mut transcripts = Vec::new();
    for transcript in transcript_iter {
        transcripts.push(transcript?);
    }

    Ok(transcripts)
}

pub fn insert_transcript(conn: &Connection, content: &str, raw_content: &str) -> Result<i64> {
    conn.execute(
        "INSERT INTO transcripts (content, raw_content) VALUES (?1, ?2)",
        params![content, raw_content],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn delete_transcript(conn: &Connection, id: i64) -> Result<()> {
    conn.execute("DELETE FROM transcripts WHERE id = ?1", params![id])?;
    Ok(())
}

pub fn get_settings(conn: &Connection) -> Result<HashMap<String, String>> {
    let mut stmt = conn.prepare("SELECT key, value FROM app_settings")?;
    let settings_iter = stmt.query_map([], |row| {
        Ok((row.get(0)?, row.get(1)?))
    })?;

    let mut map = HashMap::new();
    for setting in settings_iter {
        let (k, v): (String, String) = setting?;
        map.insert(k, v);
    }
    Ok(map)
}

pub fn update_setting(conn: &Connection, key: &str, value: &str) -> Result<()> {
    conn.execute(
        "INSERT OR REPLACE INTO app_settings (key, value) VALUES (?1, ?2)",
        params![key, value],
    )?;
    Ok(())
}
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct Profile {
    pub id: i64,
    pub name: String,
    pub system_prompt: String,
    pub icon: Option<String>,
}

pub fn get_profiles(conn: &Connection) -> Result<Vec<Profile>> {
    let mut stmt = conn.prepare("SELECT id, name, system_prompt, icon FROM transformation_profiles")?;
    let profile_iter = stmt.query_map([], |row| {
        Ok(Profile {
            id: row.get(0)?,
            name: row.get(1)?,
            system_prompt: row.get(2)?,
            icon: row.get(3)?,
        })
    })?;

    let mut profiles = Vec::new();
    for profile in profile_iter {
        profiles.push(profile?);
    }
    Ok(profiles)
}

pub fn get_active_profile(conn: &Connection) -> Result<Option<Profile>> {
    let settings = get_settings(conn)?;
    let active_id_str = settings.get("active_profile_id").cloned().unwrap_or_else(|| "1".to_string());
    let active_id: i64 = active_id_str.parse().unwrap_or(1);

    let mut stmt = conn.prepare("SELECT id, name, system_prompt, icon FROM transformation_profiles WHERE id = ?1")?;
    let mut profile_iter = stmt.query_map(params![active_id], |row| {
        Ok(Profile {
            id: row.get(0)?,
            name: row.get(1)?,
            system_prompt: row.get(2)?,
            icon: row.get(3)?,
        })
    })?;

    if let Some(profile) = profile_iter.next() {
        Ok(Some(profile?))
    } else {
        Ok(None)
    }
}

pub fn update_profile(conn: &Connection, id: i64, name: &str, prompt: &str, icon: Option<String>) -> Result<()> {
    conn.execute(
        "UPDATE transformation_profiles SET name = ?1, system_prompt = ?2, icon = ?3 WHERE id = ?4",
        params![name, prompt, icon, id],
    )?;
    Ok(())
}

pub fn create_profile(conn: &Connection, name: &str, prompt: &str, icon: Option<String>) -> Result<i64> {
    conn.execute(
        "INSERT INTO transformation_profiles (name, system_prompt, icon, is_default) VALUES (?1, ?2, ?3, 0)",
        params![name, prompt, icon],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn delete_profile(conn: &Connection, id: i64) -> Result<()> {
    // Don't allow deleting default profiles if we want to be safe, 
    // or just allow it if the user wants full control.
    // For now, let's just allow it based on ID.
    conn.execute("DELETE FROM transformation_profiles WHERE id = ?1 AND is_default = 0", params![id])?;
    Ok(())
}

pub fn get_custom_dictionary(conn: &Connection) -> Result<Vec<String>> {
    let mut stmt = conn.prepare("SELECT word FROM custom_dict")?;
    let word_iter = stmt.query_map([], |row| row.get::<_, String>(0))?;
    
    let mut words = Vec::new();
    for word in word_iter {
        words.push(word?);
    }
    Ok(words)
}

pub fn remove_from_dictionary(conn: &Connection, word: &str) -> Result<()> {
    conn.execute("DELETE FROM custom_dict WHERE word = ?1", params![word])?;
    Ok(())
}
