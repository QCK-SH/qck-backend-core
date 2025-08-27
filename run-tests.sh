#!/bin/bash

# QCK Backend Test Runner with Diesel
# Runs all tests using docker-compose.test.yml environment

echo "🚀 Starting QCK Backend Test Suite (Diesel/Axum)"
echo "================================================"

# Set PostgreSQL library path for Diesel on macOS
if [[ "$OSTYPE" == "darwin"* ]]; then
    export LIBRARY_PATH="/opt/homebrew/opt/postgresql@15/lib:$LIBRARY_PATH"
    export PKG_CONFIG_PATH="/opt/homebrew/opt/postgresql@15/lib/pkgconfig:$PKG_CONFIG_PATH"
fi

# Start test environment
echo "📦 Starting test environment..."
docker-compose -f docker-compose.test.yml up -d

# Wait for services to be healthy
echo "⏳ Waiting for services to be healthy..."
sleep 10

# Export test environment variables (or source from .env.test)
if [ -f .env.test ]; then
    echo "📋 Loading environment from .env.test"
    export $(cat .env.test | grep -v '^#' | xargs)
else
    echo "⚠️  .env.test not found, using inline variables"
    export DATABASE_URL="postgresql://qck_user:qck_password@localhost:15001/qck_test"
    export REDIS_URL="redis://localhost:15002"
    export CLICKHOUSE_URL="http://localhost:15003"
    export JWT_ACCESS_SECRET="test-access-secret-hs256-minimum-32-characters-long"
    export JWT_REFRESH_SECRET="test-refresh-secret-hs256-minimum-32-characters-long"
    export JWT_ACCESS_EXPIRY="3600"
    export JWT_REFRESH_EXPIRY="604800"
    export JWT_KEY_VERSION="1"
    export JWT_AUDIENCE="qck.sh"
    export JWT_ISSUER="qck.sh"
    export REDIS_CONNECTION_TIMEOUT="5"
    export REDIS_COMMAND_TIMEOUT="5"
    export RUST_LOG="warn"
fi

# Run tests
echo "🧪 Running tests..."
cargo test --all-features --no-fail-fast

TEST_RESULT=$?

# Show test results
if [ $TEST_RESULT -eq 0 ]; then
    echo "✅ All tests passed!"
else
    echo "❌ Some tests failed"
fi

# Optionally stop test environment
read -p "Stop test environment? (y/n) " -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]; then
    echo "🛑 Stopping test environment..."
    docker-compose -f docker-compose.test.yml down
fi

exit $TEST_RESULT