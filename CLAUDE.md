# QCK Backend - Complete Project Memory

> **This is the Single Source of Truth for the QCK Backend project**
> 
> Rust-based monolithic backend API for the QCK URL shortener platform. This document contains everything needed to work on this project independently.

## Project Context

This is the core backend service for QCK, handling all API operations, database interactions, and business logic. Built with Rust and Actix-web for maximum performance and reliability.

## 🚨 CRITICAL SAFETY RULE

**ALWAYS CHECK GIT STATUS FIRST**
Before starting ANY task that modifies files:
1. Run `git status` to check for uncommitted changes
2. If uncommitted changes exist, inform the user:
   > "⚠️ You have uncommitted changes. Any work I do might be destructive. Please commit your changes before proceeding with this task."
3. DO NOT proceed with file modifications until user confirms or commits
4. This check is MANDATORY - no exceptions

## Critical Rules

- **Package Manager**: Use `cargo` exclusively (never npm/yarn/pnpm)
- **Database Access**: All databases run via docker-compose.yml in this directory
- **Hot Reload**: Use `cargo-watch` for development
- **API URL**: http://localhost:8080 (development)
- **Testing**: Run `cargo test` before any commit
- **Formatting**: Always run `cargo fmt` before commits
- **Linting**: Always run `cargo clippy` and fix warnings

## Architecture Overview

### Service Structure
```
qck-backend/
├── src/
│   ├── main.rs              # Application entry point
│   ├── handlers/            # HTTP request handlers
│   ├── services/            # Business logic layer
│   ├── models/              # Database models
│   ├── middleware/          # Custom middleware
│   ├── utils/               # Utility functions
│   └── config/              # Configuration management
├── migrations/              # SQL migrations
├── tests/                   # Integration tests
└── docker-compose.yml       # All backend services
```

### Database Stack
- **PostgreSQL**: Main data store (users, links, settings)
- **Redis**: Cache layer and session storage
- **ClickHouse**: Analytics and event tracking
- **Adminer**: Database UI at http://localhost:8081

## Development Workflow

### Starting the Backend
```bash
# Start all services
docker-compose up -d

# View logs
docker-compose logs -f qck-api

# Stop services
docker-compose down
```

### Common Tasks

#### Adding a New Endpoint
1. Define route in @src/main.rs
2. Create handler in @src/handlers/
3. Implement business logic in @src/services/
4. Add models in @src/models/ if needed
5. Write tests in @tests/ or inline
6. Update OpenAPI spec in @api-spec.yaml

#### Database Migrations
```bash
# Create new migration
sqlx migrate add migration_name

# Run migrations
sqlx migrate run

# Revert last migration
sqlx migrate revert
```

## Code Standards

### Rust Conventions
- Use `snake_case` for functions and variables
- Use `PascalCase` for types and structs
- Prefer `Result<T, E>` for error handling
- Use `thiserror` for custom errors
- Keep handlers thin, logic in services
- Use `#[instrument]` for tracing

### Error Handling Pattern
```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ApiError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
    
    #[error("Validation error: {0}")]
    Validation(String),
    
    #[error("Not found")]
    NotFound,
}
```

### API Response Format
```rust
#[derive(Serialize)]
struct ApiResponse<T> {
    success: bool,
    data: Option<T>,
    error: Option<ErrorDetail>,
    meta: ResponseMeta,
}
```

## Testing Strategy

### Unit Tests
- Test individual functions in isolation
- Mock external dependencies
- Use `#[cfg(test)]` modules

### Integration Tests
- Test full API endpoints
- Use test database
- Clean up after tests

### Performance Tests
- Load test critical endpoints
- Monitor memory usage
- Check for memory leaks

## Performance Guidelines

- Database queries < 50ms
- API responses < 200ms
- Use connection pooling
- Implement query caching
- Batch operations when possible
- Use indexes on foreign keys

## Security Checklist

- [ ] Input validation on all endpoints
- [ ] SQL injection prevention (parameterized queries)
- [ ] Rate limiting implemented
- [ ] JWT validation on protected routes
- [ ] Secrets in environment variables
- [ ] CORS properly configured
- [ ] Error messages don't leak sensitive info

## Common Issues & Solutions

### Database Connection Issues
- Check docker-compose is running
- Verify DATABASE_URL is correct
- Check PostgreSQL logs: `docker-compose logs postgres`

### Compilation Errors
- Run `cargo clean` and rebuild
- Update dependencies: `cargo update`
- Check for breaking changes in Cargo.toml

### Performance Problems
- Enable debug logging
- Check slow query logs
- Profile with `cargo flamegraph`
- Monitor Redis hit rate

## Environment Variables

Required in @.env:
```bash
DATABASE_URL=postgresql://qck_user:qck_password@localhost:5432/qck_db
REDIS_URL=redis://localhost:6379
CLICKHOUSE_URL=http://localhost:8123
JWT_SECRET=your-secret-here
RUST_LOG=info,qck_backend=debug
```

## API Documentation

- OpenAPI spec: @api-spec.yaml
- Postman collection: @postman/qck-api.json
- API docs will be generated in @apidocs/

## Deployment Notes

### Docker Build
```bash
docker build -t qck-backend .
docker run -p 8080:8080 qck-backend
```

### Production Checklist
- [ ] Environment variables set
- [ ] Database migrations run
- [ ] Redis cache warmed
- [ ] Health checks passing
- [ ] Monitoring configured
- [ ] Logs aggregated

## Linear Integration

### Team Configuration
- **Linear Team ID**: `8557f533-649e-476c-9d20-efe571bfe69c`
- Issue limit: 150 per request
- Always check for existing issues before creating new ones
- Update issue progress and completion status regularly

### Issue Status Workflow
**IMPORTANT**: Never mark issues as "Done" directly. Always use "waiting for review" when completing work.

- **Todo** - Unstarted tasks
- **In Progress** - When actively working on something
- **In Review** - When code is in review
- **QA** - When it needs testing
- **waiting for review** - When work is complete and needs human review
- **waiting for merge** - When approved and ready to merge
- **Done** - Only after human verification and approval
- **Backlog** - Backlog items
- **Canceled** - Canceled tasks
- **Duplicate** - Duplicate issues

### Issue Management Rules
- Always check for existing Linear issues before starting work
- Update issue status when starting work
- Link commits to Linear issues using issue ID
- Mark as "waiting for review" when complete
- Include screenshots/logs for bug fixes
- Document API changes in issue comments

## Git Workflow

### Branch Naming
- `feature/description` - New features
- `fix/description` - Bug fixes
- `refactor/description` - Code refactoring
- `docs/description` - Documentation updates
- `test/description` - Test additions/changes
- `perf/description` - Performance improvements

### Commit Conventions
Use conventional commits for clear history:
- `feat:` - New feature
- `fix:` - Bug fix
- `docs:` - Documentation changes
- `style:` - Code style changes (formatting, etc.)
- `refactor:` - Code refactoring
- `test:` - Test additions or changes
- `perf:` - Performance improvements
- `build:` - Build system changes
- `ci:` - CI/CD changes

### PR Requirements
- Create PR for all changes
- Include Linear issue ID in PR title
- Pass all CI checks
- Get at least one review
- Squash and merge to main

## Memory Breadcrumb System

### Directory Structure
Every significant change MUST be documented in @.claude/memory/:

```
.claude/memory/
├── features/           # Feature implementations
│   └── YYYY-MM-DD-feature-name.md
├── tasks/             # Individual tasks completed
│   └── YYYY-MM-DD-task-description.md
├── fixes/             # Bug fixes and issues resolved
│   └── YYYY-MM-DD-fix-description.md
├── architecture/      # Architecture decisions and changes
│   └── YYYY-MM-DD-architecture-change.md
└── index.md          # Master index of all changes
```

### Memory File Template
```markdown
# [Type]: [Brief Description]
Date: YYYY-MM-DD HH:MM
Linear Issue: [ID or N/A]
Status: [completed/in-progress/blocked]

## Context
- **Request**: [What was asked]
- **Purpose**: [Why it's needed]
- **Dependencies**: [Related features/tasks]

## Implementation
### Files Modified
- `path/to/file.rs:120-145` - Added authentication middleware
- `path/to/other.ts:50-75` - Updated API client

### Key Code Changes
\```rust
// Example of significant code added
fn authenticate_user() -> Result<User> {
    // Implementation
}
\```

## How It Works
1. [Step-by-step explanation]
2. [Data flow description]
3. [Integration details]

## Testing
- Command: `cargo test auth_tests`
- Expected: All tests pass
- Coverage: 85%

## Rollback
- Revert commit: [hash]
- Remove migration: `down.sql`
- Restore config: [details]

## Notes
- [Any additional context]
- [Lessons learned]
- [Future improvements]
```

### Documentation Requirements
For EVERY task/feature:
1. Create memory file before starting
2. Update as you progress
3. Include all modified files with line numbers
4. Document design decisions
5. Add rollback instructions
6. Update index.md

---

*This file is the complete, self-contained documentation for the qck-backend project*
*Always keep this documentation updated with changes*