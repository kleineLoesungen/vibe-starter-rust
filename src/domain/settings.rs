//! Schluessel-Wert-Speicher fuer Werte, die zur Laufzeit entstehen.
//!
//! Gedacht fuer wenige, kleine Werte — etwa das Ergebnis des Update-Abgleichs.
//! Alles, wonach gesucht, sortiert oder gefiltert wird, gehoert in eine eigene
//! Tabelle mit eigenen Spalten.

use crate::error::Error;
use sqlx::PgPool;

pub async fn get(db: &PgPool, key: &str) -> Result<Option<String>, Error> {
    let row: Option<(String,)> = sqlx::query_as("SELECT value FROM settings WHERE key = $1")
        .bind(key)
        .fetch_optional(db)
        .await?;
    Ok(row.map(|(value,)| value))
}

pub async fn set(db: &PgPool, key: &str, value: &str) -> Result<(), Error> {
    sqlx::query(
        "INSERT INTO settings (key, value) VALUES ($1, $2)
         ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value, updated_at = now()",
    )
    .bind(key)
    .bind(value)
    .execute(db)
    .await?;
    Ok(())
}
