#!/usr/bin/env python
"""
Database migration script - Syncs schema with current SQLAlchemy models.
Handles SQLite limitations with defaults like CURRENT_TIMESTAMP.

Usage:
  python migrate_db.py                              # default: data/userserver.db
  python migrate_db.py --db /path/to/custom.db
"""

import sqlite3
import sys
from pathlib import Path
from datetime import datetime

def parse_db_path() -> Path:
    custom_db = None
    for i, arg in enumerate(sys.argv[1:], 1):
        if arg.startswith("--db="):
            custom_db = arg.split("=", 1)[1]
            break
        elif arg == "--db" and i < len(sys.argv) - 1:
            custom_db = sys.argv[i + 1]
            break

    basedir = Path(__file__).parent.absolute()

    if custom_db:
        db_path = Path(custom_db)
        if not db_path.is_absolute():
            db_path = basedir / db_path
        print(f"Using custom database: {db_path}")
    else:
        db_path = basedir / "data" / "userserver.db"
        print(f"Using default database: {db_path}")

    return db_path


def get_db_connection(db_path: Path) -> sqlite3.Connection:
    db_path.parent.mkdir(parents=True, exist_ok=True)
    conn = sqlite3.connect(str(db_path))
    conn.row_factory = sqlite3.Row
    return conn


def table_exists(conn: sqlite3.Connection, table_name: str) -> bool:
    cursor = conn.cursor()
    cursor.execute("SELECT name FROM sqlite_master WHERE type='table' AND name=?", (table_name,))
    return cursor.fetchone() is not None


def get_existing_columns(conn: sqlite3.Connection, table_name: str) -> set:
    if not table_exists(conn, table_name):
        return set()
    cursor = conn.cursor()
    try:
        cursor.execute(f"PRAGMA table_info({table_name})")
        return {row[1] for row in cursor.fetchall()}
    finally:
        cursor.close()


def add_column(conn: sqlite3.Connection, table_name: str, column_name: str, column_def: str, default_value=None):
    """Add column safely. If it has a dynamic default (like CURRENT_TIMESTAMP), handle it separately."""
    cursor = conn.cursor()
    try:
        # Add column without dynamic default first
        clean_def = column_def.split(" DEFAULT ")[0] if " DEFAULT " in column_def.upper() else column_def
        cursor.execute(f"ALTER TABLE {table_name} ADD COLUMN {column_name} {clean_def}")
        conn.commit()
        print(f"✓ Added column: {table_name}.{column_name}")

        # Backfill default value if provided
        if default_value is not None:
            cursor.execute(f"UPDATE {table_name} SET {column_name} = ? WHERE {column_name} IS NULL", (default_value,))
            conn.commit()
            print(f"   → Backfilled default values for existing rows")

        return True
    except sqlite3.OperationalError as e:
        err = str(e).lower()
        if "already exists" in err:
            print(f"• Column already exists: {table_name}.{column_name}")
            return True
        else:
            print(f"✗ Failed to add {table_name}.{column_name}: {e}")
            return False
    finally:
        cursor.close()


def migrate(db_path: Path):
    print(f"\n{'='*70}")
    print("DATABASE MIGRATION TOOL (SQLite-safe)")
    print(f"Target: {db_path}")
    print(f"{'='*70}\n")

    if not db_path.exists():
        print("⚠️  Database does not exist yet. It will be created on first app run.")
        return

    conn = get_db_connection(db_path)
    added = 0

    try:
        # ====================== User Table ======================
        print("Checking table: user")
        if table_exists(conn, "user"):
            cols = get_existing_columns(conn, "user")

            user_migrations = [
                ("email", "VARCHAR(120)"),
                ("otp_secret", "VARCHAR(32)"),
                ("is_2fa_enabled", "BOOLEAN DEFAULT 0"),
                ("is_admin", "BOOLEAN DEFAULT 0"),
                ("is_deleted", "BOOLEAN DEFAULT 0"),
                ("display_name", "VARCHAR(100)"),
                ("theme", "VARCHAR(20) DEFAULT 'dark'"),
                ("accent_color", "VARCHAR(7) DEFAULT '#7eafff'"),
                ("bio", "TEXT"),
                ("profile_pic", "TEXT"),
                ("Status_Text", "VARCHAR(32)"),
                ("Status_type", "INTEGER"),
                ("created_at", "DATETIME", datetime.now()),   # special handling
            ]

            for mig in user_migrations:
                if len(mig) == 2:
                    name, definition = mig
                    default_val = None
                else:
                    name, definition, default_val = mig

                if name not in cols:
                    if add_column(conn, "user", name, definition, default_val):
                        added += 1
        else:
            print("   Table 'user' does not exist yet.")

        # ====================== UserSession Table ======================
        print("\nChecking table: usersession")
        if table_exists(conn, "usersession"):
            cols = get_existing_columns(conn, "usersession")

            session_migrations = [
                ("token", "VARCHAR(64) UNIQUE NOT NULL"),
                ("device_info", "VARCHAR(255)"),
                ("last_active", "DATETIME", datetime.now()),
                ("expires_at", "DATETIME NOT NULL"),
            ]

            for mig in session_migrations:
                if len(mig) == 2:
                    name, definition = mig
                    default_val = None
                else:
                    name, definition, default_val = mig

                if name not in cols:
                    if add_column(conn, "usersession", name, definition, default_val):
                        added += 1
        else:
            print("   Table 'usersession' does not exist yet (will be created by app).")

        # ====================== Message Table ======================
        print("\nChecking table: message")
        if table_exists(conn, "message"):
            cols = get_existing_columns(conn, "message")

            message_migrations = [
                ("sender", "VARCHAR(100)"),
                ("receiver", "VARCHAR(100)"),
                ("text", "TEXT"),
                ("validation_key", "VARCHAR(50)"),
                ("timestamp", "DATETIME", datetime.now()),
                ("is_read", "BOOLEAN DEFAULT 0"),
            ]

            for mig in message_migrations:
                if len(mig) == 2:
                    name, definition = mig
                    default_val = None
                else:
                    name, definition, default_val = mig

                if name not in cols:
                    if add_column(conn, "message", name, definition, default_val):
                        added += 1
        else:
            print("   Table 'message' does not exist yet.")

        # ====================== InviteCode Table ======================
        print("\nChecking table: invitecode")
        if table_exists(conn, "invitecode"):
            cols = get_existing_columns(conn, "invitecode")

            invite_migrations = [
                ("code", "VARCHAR(64) UNIQUE NOT NULL"),
                ("created_by", "INTEGER"),
                ("used_by", "INTEGER"),
                ("created_at", "DATETIME", datetime.now()),
                ("used_at", "DATETIME"),
                ("expires_at", "DATETIME"),
            ]

            for mig in invite_migrations:
                if len(mig) == 2:
                    name, definition = mig
                    default_val = None
                else:
                    name, definition, default_val = mig

                if name not in cols:
                    if add_column(conn, "invitecode", name, definition, default_val):
                        added += 1
        else:
            print("   Table 'invitecode' does not exist yet.")

    finally:
        conn.close()

    print(f"\n✅ Migration finished! {added} column(s) added.")
    print("You can now run your Flask app.\n")


if __name__ == "__main__":
    db_path = parse_db_path()
    migrate(db_path)