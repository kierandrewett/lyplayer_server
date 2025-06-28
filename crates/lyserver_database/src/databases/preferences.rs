use std::sync::Arc;

use lyserver_shared_data::{LYServerSharedData, LYServerSharedDataDirectories as _};
use sqlx::{any, Pool, Sqlite};

use crate::database::{LYServerDatabase, LYServerDatabaseConnection, LYServerDatabaseLifecycle};

const PREFERENCES_DB_SCHEMA_VERSION: u32 = 1;

pub struct LYServerPreferencesDatabase {
    db: LYServerDatabase,
}

impl LYServerDatabaseConnection for LYServerPreferencesDatabase {
    async fn get_pool(&self) -> anyhow::Result<Arc<Pool<Sqlite>>> {
        self.db.get_pool().await
    }
}

impl LYServerDatabaseLifecycle for LYServerPreferencesDatabase {
    async fn connect(&mut self) -> anyhow::Result<()> {
        self.db.connect().await?;

        self.maybe_migrate_schema().await?;

        Ok(())
    }

    async fn disconnect(&mut self) -> anyhow::Result<()> {
        self.db.disconnect().await
    }

    async fn health_check(&self) -> anyhow::Result<()> {
        self.db.health_check().await
    }
}

impl LYServerPreferencesDatabase {
    pub fn new(shared_data: Arc<LYServerSharedData>) -> Self {
        let db_path = shared_data.resolve_data_path_str("preferences.db");

        Self {
            db: LYServerDatabase::new(db_path),
        }
    }

    pub async fn maybe_migrate_schema(&self) -> anyhow::Result<()> {
        let current_version = self.db.get_schema_version().await?;

        log::info!("Migrating preferences database schema from version {} to {}", current_version, PREFERENCES_DB_SCHEMA_VERSION);

        if current_version < 1 {
            self.db.with_db_connection(|pool| {
                Box::pin(async move {
                    // Ensure we have foreign key support enabled
                    sqlx::query("PRAGMA foreign_keys = ON")
                        .execute(pool)
                        .await
                        .map_err(|e| anyhow::anyhow!("Failed to enable foreign keys: {}", e))?;

                    // Create the preference_native_type_lookup table
                    sqlx::query("CREATE TABLE IF NOT EXISTS preference_native_type_lookup (
                        id INTEGER PRIMARY KEY NOT NULL,
                        type TEXT NOT NULL
                    )")
                        .execute(pool)
                        .await
                        .map_err(|e| anyhow::anyhow!("Failed to create preference_native_type_lookup table: {}", e))?;

                    // Create the native types lookup entries
                    sqlx::query("INSERT OR IGNORE INTO preference_native_type_lookup (id, type) VALUES
                        (0, 'null'),
                        (1, 'i32'),
                        (2, 'f32'),
                        (3, 'u32'),
                        (4, 'bool'),
                        (5, 'str'),
                        (6, 'json')
                    ")
                        .execute(pool)
                        .await
                        .map_err(|e| anyhow::anyhow!("Failed to insert native types: {}", e))?;

                    // Create the preferences table
                    sqlx::query("CREATE TABLE IF NOT EXISTS preferences (
                        key TEXT PRIMARY KEY NOT NULL,
                        value TEXT,
                        native_type_id INTEGER NOT NULL,
                        is_locked INTEGER NOT NULL DEFAULT (0),
                        created_at DATETIME DEFAULT (CURRENT_TIMESTAMP),
                        updated_at DATETIME DEFAULT (CURRENT_TIMESTAMP),
                        CHECK (is_locked IN (0, 1)),
                        FOREIGN KEY (native_type_id) REFERENCES preference_native_type_lookup (id)
                    )")
                        .execute(pool)
                        .await
                        .map_err(|e| anyhow::anyhow!("Failed to create preferences table: {}", e))?;

                    // Create the preferences trigger to update the updated_at field
                    sqlx::query("CREATE TRIGGER IF NOT EXISTS preferences_update_trigger 
                        AFTER UPDATE ON preferences 
                        BEGIN 
                            UPDATE preferences 
                            SET updated_at = CURRENT_TIMESTAMP 
                            WHERE key = NEW.key; 
                        END
                    ")
                        .execute(pool)
                        .await
                        .map_err(|e| anyhow::anyhow!("Failed to create preferences update trigger: {}", e))?;

                    Ok(())
                })
            }).await?;
        }

        if current_version < PREFERENCES_DB_SCHEMA_VERSION {
            self.db.set_schema_version(PREFERENCES_DB_SCHEMA_VERSION).await?;
        }

        Ok(())
    }
}