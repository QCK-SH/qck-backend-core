#!/bin/bash

# Database migration script for QCK Backend
# Requires sqlx-cli to be installed: cargo install sqlx-cli --no-default-features --features postgres

set -e

# Load environment variables safely
if [ -f .env ]; then
    set -a
    source .env
    set +a
fi

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Check if sqlx is installed
if ! command -v sqlx &> /dev/null; then
    echo -e "${RED}Error: sqlx-cli is not installed${NC}"
    echo "Install it with: cargo install sqlx-cli --no-default-features --features postgres"
    exit 1
fi

# Check DATABASE_URL is set
if [ -z "$DATABASE_URL" ]; then
    echo -e "${RED}Error: DATABASE_URL environment variable is not set${NC}"
    echo "Please set it in your .env file or environment"
    exit 1
fi

# Function to display usage
usage() {
    echo "Usage: $0 [command] [args]"
    echo ""
    echo "Commands:"
    echo "  create <name>    Create a new migration with the given name"
    echo "  run              Run all pending migrations"
    echo "  revert           Revert the last migration"
    echo "  reset            Drop and recreate the database, then run all migrations"
    echo "  info             Show migration status"
    echo "  list             List all migrations"
    echo ""
    echo "Examples:"
    echo "  $0 create add_user_preferences"
    echo "  $0 run"
    echo "  $0 revert"
}

# Main command handler
case "$1" in
    create)
        if [ -z "$2" ]; then
            echo -e "${RED}Error: Migration name required${NC}"
            echo "Usage: $0 create <migration_name>"
            exit 1
        fi
        echo -e "${GREEN}Creating migration: $2${NC}"
        sqlx migrate add -r "$2"
        echo -e "${GREEN}✓ Migration created successfully${NC}"
        echo "Edit the new migration files in the migrations/ directory"
        ;;
        
    run)
        echo -e "${GREEN}Running pending migrations...${NC}"
        sqlx migrate run
        echo -e "${GREEN}✓ Migrations completed successfully${NC}"
        ;;
        
    revert)
        echo -e "${YELLOW}Reverting last migration...${NC}"
        sqlx migrate revert
        echo -e "${GREEN}✓ Migration reverted successfully${NC}"
        ;;
        
    reset)
        echo -e "${RED}WARNING: This will drop and recreate the database!${NC}"
        read -p "Are you sure? (y/N) " -n 1 -r
        echo
        if [[ $REPLY =~ ^[Yy]$ ]]; then
            echo -e "${YELLOW}Resetting database...${NC}"
            # Removed -y flag to require explicit confirmation from sqlx as well
            # This provides double confirmation for safety in production
            sqlx database reset
            echo -e "${GREEN}✓ Database reset completed${NC}"
        else
            echo "Reset cancelled"
        fi
        ;;
        
    info)
        echo -e "${GREEN}Migration status:${NC}"
        sqlx migrate info
        ;;
        
    list)
        echo -e "${GREEN}Available migrations:${NC}"
        ls -la migrations/*.sql 2>/dev/null || echo "No migrations found"
        ;;
        
    *)
        usage
        exit 1
        ;;
esac