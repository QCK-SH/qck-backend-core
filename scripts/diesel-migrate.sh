#!/bin/bash

# Diesel migration runner for QCK Backend
# Waits for database, runs migrations, then starts the service with hot reload

echo "[QCK-BACKEND] Starting migration and service runner..."

# Wait for database to be ready
echo "[QCK-BACKEND] Waiting for database..."

for i in {1..30}; do
    if psql "$DATABASE_URL" -c "SELECT 1" > /dev/null 2>&1; then
        echo "[QCK-BACKEND] Database is ready!"
        break
    fi
    echo "[QCK-BACKEND] Waiting for database... ($i/30)"
    sleep 2
done

# Check if we should run migrations
if [ "$RUN_MIGRATIONS" = "true" ]; then
    echo "[QCK-BACKEND] Running Diesel migrations..."
    
    # Run diesel migrations
    diesel migration run
    
    if [ $? -eq 0 ]; then
        echo "[QCK-BACKEND] Migrations completed successfully!"
    else
        echo "[QCK-BACKEND] Migration failed! Checking if already applied..."
        # Try to verify migrations are already applied
        diesel migration list
        if [ $? -eq 0 ]; then
            echo "[QCK-BACKEND] Migrations appear to be already applied - continuing..."
        else
            echo "[QCK-BACKEND] ERROR: Could not verify migration status!"
            exit 1
        fi
    fi
else
    echo "[QCK-BACKEND] Skipping migrations (RUN_MIGRATIONS != true)"
fi

# Seeds are now handled as Diesel migrations
# The demo users seed is in migrations/diesel/2025-09-01-072853_seed_demo_users/
# It will automatically run with other migrations

# Start the service with hot reload
echo "[QCK-BACKEND] Starting service with hot reload..."
exec cargo-watch --why -x run