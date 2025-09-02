#!/bin/bash

# Auto-seeding script for development and staging
# This runs automatically when the API starts in non-production environments

# Check if we should run seeding
# Using ENVIRONMENT variable from .env files
if [[ "$ENVIRONMENT" == "production" ]] || [[ "$NODE_ENV" == "production" ]] || [[ "$DISABLE_SEEDING" == "true" ]]; then
    echo "Seeding disabled for this environment"
    exit 0
fi

# Check if seed has already run (to avoid duplicates)
SEED_MARKER="/tmp/.qck_seeded_$(date +%Y%m%d)"
if [[ -f "$SEED_MARKER" ]]; then
    echo "Database already seeded today"
    exit 0
fi

# Wait for API to be healthy
echo "Waiting for API to be ready..."
for i in {1..30}; do
    if curl -s http://localhost:8080/v1/health > /dev/null 2>&1; then
        echo "API is ready"
        break
    fi
    sleep 2
done

# Run the seed script
if [[ -f "/app/seed_staging.sh" ]]; then
    echo "Running database seed..."
    /app/seed_staging.sh
    
    # Mark as seeded
    touch "$SEED_MARKER"
    echo "Seeding completed"
else
    echo "Seed script not found"
fi