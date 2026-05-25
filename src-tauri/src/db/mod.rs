pub mod schema;
pub mod migrations;

use rusqlite::{Connection, Result};
use std::path::Path;

pub struct Database {
    conn: Connection,
}

impl Database {
    pub fn new(path: &str) -> Result<Self> {
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        Ok(Self { conn })
    }

    pub fn init(&self) -> Result<()> {
        // Create tables
        self.conn.execute_batch(schema::SCHEMA)?;
        
        // Run migrations
        migrations::run_migrations(&self.conn)?;
        
        // Insert default settings
        self.init_settings()?;
        
        Ok(())
    }

    fn init_settings(&self) -> Result<()> {
        let defaults = [
            ("root_dir", ""),
            ("view_mode", "grid"),
            ("sort_by", "updated"),
            ("sort_order", "desc"),
            ("reader_fit", "height"),
            ("reader_bg", "#1a1a1a"),
            ("auto_scan_interval", "0"),
            ("scan_depth", "1"),
            ("page_direction", "rtl"),
            ("theme", "dark"),
        ];

        for (key, value) in defaults {
            self.conn.execute(
                "INSERT OR IGNORE INTO settings (key, value) VALUES (?1, ?2)",
                (key, value),
            )?;
        }

        Ok(())
    }

    pub fn get_conn(&self) -> &Connection {
        &self.conn
    }

    pub fn get_data_dir() -> std::path::PathBuf {
        if let Ok(dir) = std::env::var("DATA_DIR") {
            std::path::PathBuf::from(dir)
        } else {
            dirs::data_dir()
                .unwrap_or_else(|| std::path::PathBuf::from("."))
                .join("MangaViewer")
                .join("data")
        }
    }
}
