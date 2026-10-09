-- Allgemeiner Schluessel-Wert-Speicher fuer Werte, die zur Laufzeit entstehen
-- und keine eigene Tabelle rechtfertigen.
--
-- Erster Eintrag: 'update_check' — Ergebnis des Abgleichs mit UPDATE_CHECK_URL.
--
-- `value` ist TEXT und nicht JSONB: Fuer JSONB braeuchte sqlx das Feature
-- `json`, und an den festgenagelten sqlx-Einstellungen soll ohne Not nichts
-- geaendert werden. Geparst wird mit serde_json.

CREATE TABLE settings (
    key        TEXT        PRIMARY KEY,
    value      TEXT        NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
