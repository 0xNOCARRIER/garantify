use sqlx::PgPool;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DbError {
    #[error("Erreur de connexion à la base de données : {0}")]
    Connection(#[from] sqlx::Error),
}

pub async fn create_pool(database_url: &str) -> Result<PgPool, DbError> {
    let pool = PgPool::connect(database_url).await?;
    Ok(pool)
}
