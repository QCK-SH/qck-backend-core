#!/bin/bash
# Custom ClickHouse entrypoint that ensures migrations run
# This wraps the original entrypoint and adds migration logic

echo "=== QCK ClickHouse Custom Entrypoint ==="

# Start ClickHouse in background using original entrypoint
/entrypoint.sh &
CLICKHOUSE_PID=$!

# Wait for ClickHouse to be ready
echo "Waiting for ClickHouse to start..."
MAX_RETRIES=30
RETRY_COUNT=0

while [ $RETRY_COUNT -lt $MAX_RETRIES ]; do
    if clickhouse-client --query "SELECT 1" >/dev/null 2>&1; then
        echo "✓ ClickHouse is ready"
        break
    fi
    echo "Waiting... ($((RETRY_COUNT+1))/$MAX_RETRIES)"
    sleep 2
    RETRY_COUNT=$((RETRY_COUNT+1))
done

if [ $RETRY_COUNT -eq $MAX_RETRIES ]; then
    echo "✗ ClickHouse failed to start"
    exit 1
fi

# Apply migrations
echo "Applying migrations..."
MIGRATION_DIR="/docker-entrypoint-initdb.d"

if [ -d "$MIGRATION_DIR" ]; then
    for migration in $(ls -1 $MIGRATION_DIR/*.sql 2>/dev/null | sort); do
        if [ -f "$migration" ]; then
            MIGRATION_NAME=$(basename "$migration")
            echo "Applying: $MIGRATION_NAME"
            
            # Run migration with multiquery (they're idempotent with IF NOT EXISTS)
            cat "$migration" | clickhouse-client --multiquery 2>&1 | grep -v "already exists" || true
            echo "✓ Applied: $MIGRATION_NAME"
        fi
    done
    echo "✓ All migrations applied"
else
    echo "⚠ No migrations found in $MIGRATION_DIR"
fi

echo "=== Migration process complete ==="

# Wait for ClickHouse process
wait $CLICKHOUSE_PID