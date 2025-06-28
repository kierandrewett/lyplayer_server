use std::{path::PathBuf, sync::Arc};

use anyhow::Ok;
use futures::future::BoxFuture;
use sqlx::{sqlite::{SqliteArgumentValue, SqliteArguments, SqlitePoolOptions, SqliteRow}, Encode, Pool, Row as _, Sqlite, Type, Arguments as _};
use tokio::sync::Mutex;

pub trait LYServerDatabaseConnection {
    async fn get_pool(&self) -> anyhow::Result<Arc<Pool<Sqlite>>>;
}

pub trait LYServerDatabaseLifecycle {
    async fn connect(&mut self) -> anyhow::Result<()>;
    async fn disconnect(&mut self) -> anyhow::Result<()>;
    async fn health_check(&self) -> anyhow::Result<()>;
}

pub struct LYServerDatabase {
    db_connection_uri: String,

    pool: Arc<Mutex<Option<Pool<Sqlite>>>>,
}

impl LYServerDatabase {
    pub fn new(db_path: PathBuf) -> Self {
        if !db_path.exists() {
            std::fs::write(&db_path, b"")
                .expect("Failed to create database file");
        }

        let db_connection_uri = format!("sqlite://{}", db_path.display().to_string());

        Self {
            db_connection_uri,
            pool: Arc::new(Mutex::new(None)),
        }
    }

    pub async fn get_schema_version(&self) -> anyhow::Result<u32> {
        if let Some(pool) = &*self.pool.lock().await {
            let row: SqliteRow = sqlx::query("PRAGMA user_version")
                .fetch_one(pool)
                .await?;

            let version: u32 = row.get(0);
            Ok(version)
        } else {
            Err(anyhow::anyhow!("Database connection pool is not initialized."))
        }
    }

    pub async fn set_schema_version(&self, version: u32) -> anyhow::Result<()> {
        if let Some(pool) = &*self.pool.lock().await {
            let query = format!("PRAGMA user_version = {}", version);

            sqlx::query(query.as_str())
                .execute(pool)
                .await?;

            Ok(())
        } else {
            Err(anyhow::anyhow!("Database connection pool is not initialized."))
        }
    }

    pub async fn with_db_connection<T>(
        &self,
        f: impl for<'a> FnOnce(&'a Pool<Sqlite>) -> BoxFuture<'a, anyhow::Result<T>>,
    ) -> anyhow::Result<T> {
        let pool = self.pool.lock().await;
        if let Some(pool) = &*pool {
            f(pool).await
        } else {
            Err(anyhow::anyhow!("Database connection pool is not initialized."))
        }
    }
}

impl LYServerDatabaseConnection for LYServerDatabase {
    async fn get_pool(&self) -> anyhow::Result<Arc<Pool<Sqlite>>> {
            let pool = self.pool.lock().await;
            if let Some(pool) = &*pool {
                Ok(Arc::new(pool.clone()))
            } else {
                Err(anyhow::anyhow!("Database connection pool is not initialized."))
            }
        }
}

impl LYServerDatabaseLifecycle for LYServerDatabase {
    async fn connect(&mut self) -> anyhow::Result<()> {
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect(self.db_connection_uri.as_str())
            .await?;

        let pool = Arc::new(Mutex::new(Some(pool)));
        self.pool = pool;

        log::info!("Connected to database at '{}'", self.db_connection_uri);

        Ok(())
    }

    async fn disconnect(&mut self) -> anyhow::Result<()> {
        if let Some(pool) = &*self.pool.lock().await {
            pool.close().await;

            log::info!("Disconnected from database at '{}'", self.db_connection_uri);
        }

        self.pool.lock().await.take();

        Ok(())
    }

    async fn health_check(&self) -> anyhow::Result<()> {
        if let Some(pool) = &*self.pool.lock().await {
            sqlx::query("SELECT 1").execute(pool).await?;
            
            Ok(())
        } else {
            return Err(anyhow::anyhow!("Database connection pool is not initialized."));
        }
    }
}