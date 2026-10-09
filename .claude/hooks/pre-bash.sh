#!/usr/bin/env bash
# Prueft jeden Shell-Befehl, den ein KI-Assistent ausfuehren will.
#
# Zwei Aufgaben:
#
#   1. Zerstoererische Befehle abweisen (Datenbank loeschen, Volume entfernen).
#   2. Vor jedem `git commit` `just check` ausfuehren. Es gibt keine CI —
#      das ist die einzige Stelle, an der ein roter Stand sicher auffaellt.
#
# Rueckgabe an Claude Code: JSON auf stdout mit permissionDecision "deny",
# sonst stilles Ende mit Code 0 (dann greift die normale Rechtepruefung).
#
# Grundsatz bei eigenen Fehlern: durchlassen, nicht blockieren. Ein kaputter
# Hook darf die Arbeit nicht unmoeglich machen — er meldet sich auf stderr.
set -uo pipefail

PROJEKT="${CLAUDE_PROJECT_DIR:-$(cd "$(dirname "$0")/../.." && pwd)}"

# --- Eingabe lesen ---------------------------------------------------------

EINGABE="$(cat)"

if ! command -v python3 >/dev/null 2>&1; then
  echo "pre-bash.sh: python3 fehlt, Befehl wird nicht geprueft." >&2
  exit 0
fi

BEFEHL="$(printf '%s' "$EINGABE" | python3 -I -c '
import json, sys
try:
    print(json.load(sys.stdin).get("tool_input", {}).get("command", ""))
except Exception:
    print("")
' 2>/dev/null)"

[ -n "$BEFEHL" ] || exit 0

# --- Antwortbausteine ------------------------------------------------------

ablehnen() {
  GRUND="$1" python3 -I -c '
import json, os
print(json.dumps({"hookSpecificOutput": {
    "hookEventName": "PreToolUse",
    "permissionDecision": "deny",
    "permissionDecisionReason": os.environ["GRUND"],
}}))'
  exit 0
}

enthaelt() { printf '%s' "$BEFEHL" | grep -Eqi "$1"; }

# --- 1. Zerstoererische Befehle -------------------------------------------

# `docker compose down -v` und `just db-reset` loeschen das Datenbank-Volume
# mitsamt aller lokalen Konten. In einem noch nicht umbenannten Klon des Kits
# traefe es sogar die Datenbank des Kits selbst.
if enthaelt 'docker +compose.*down.*(-v|--volumes)' || enthaelt '(^| )just +db-reset( |$)'; then
  ablehnen "Dieser Befehl löscht das Datenbank-Volume mitsamt aller lokalen Daten. Bitte selbst im Terminal ausführen, wenn das wirklich gewollt ist."
fi

if enthaelt 'docker +volume +rm'; then
  ablehnen "Ein Docker-Volume zu entfernen löscht Daten unwiderruflich. Bitte selbst im Terminal ausführen."
fi

# DROP/TRUNCATE nur dann abweisen, wenn sie an eine laufende Datenbank gehen.
# In Migrationsdateien sind sie voellig normal.
if enthaelt 'psql|docker +compose +exec.*psql'; then
  if enthaelt 'drop +database|drop +table|truncate +table|truncate +[a-z_]+'; then
    ablehnen "Dieser Befehl verwirft Daten in einer laufenden Datenbank. Schemaänderungen gehören in eine neue Migration unter migrations/."
  fi
fi

# --- 2. Vor dem Commit pruefen --------------------------------------------

# Nur echte Commits abfangen, keine Abfragen wie `git commit --dry-run`.
if enthaelt '(^| |;|&&|\|\|)git +(-[^ ]+ +)*commit( |$)' && ! enthaelt 'dry-run'; then
  if [ ! -f "$PROJEKT/Justfile" ]; then
    exit 0
  fi
  if ! command -v just >/dev/null 2>&1; then
    echo "pre-bash.sh: just fehlt, Commit wird nicht geprueft." >&2
    exit 0
  fi

  AUSGABE="$(cd "$PROJEKT" && just check 2>&1)"
  if [ $? -eq 0 ]; then
    exit 0
  fi

  # Haeufigster Fall: Die Datenbank laeuft nicht. Die Meldung von sqlx
  # ("PoolTimedOut") sagt das niemandem — hier steht, was zu tun ist.
  if printf '%s' "$AUSGABE" | grep -q "PoolTimedOut"; then
    ablehnen "just check kommt nicht an die Datenbank (PoolTimedOut). Erst 'just db-up' ausführen (und prüfen, ob Docker läuft), dann erneut committen."
  fi

  LETZTE="$(printf '%s' "$AUSGABE" | tail -n 25)"
  ablehnen "just check ist rot — nicht committen, bevor das behoben ist. Letzte Zeilen:

$LETZTE"
fi

exit 0
