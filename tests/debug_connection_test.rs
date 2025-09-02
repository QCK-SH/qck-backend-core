// Debug test to isolate connection issue
use qck_backend::db::{create_diesel_pool, DieselDatabaseConfig};

#[tokio::test]
async fn test_basic_database_connection() {
    // Load environment
    dotenv::from_filename("../.env.dev").ok();

    println!("DATABASE_URL: {}", std::env::var("DATABASE_URL").unwrap());

    // Try to create database pool
    let db_config = DieselDatabaseConfig::default();
    match create_diesel_pool(db_config).await {
        Ok(pool) => {
            println!("Database pool created successfully");

            // Try to get a connection
            match pool.get().await {
                Ok(_conn) => println!("Got database connection successfully"),
                Err(e) => println!("Failed to get connection: {}", e),
            }
        },
        Err(e) => println!("Failed to create database pool: {}", e),
    }
}
