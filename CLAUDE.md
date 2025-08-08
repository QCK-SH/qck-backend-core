# QCK Backend - Complete Project Memory

> **This is the Single Source of Truth for the QCK Backend project**
> 
> Rust-based monolithic backend API for the QCK URL shortener platform. This document contains everything needed to work on this project independently.

## Project Context

This is the core backend service for QCK, handling all API operations, database interactions, and business logic. Built with Rust and Actix-web for maximum performance and reliability.

## 🤖 CRITICAL: Task Delegation Protocol

**MANDATORY: Always use specialized subagents via the Task tool instead of doing work directly.**

For ANY backend development task, you MUST:
1. Identify the appropriate specialized subagent
2. Use the Task tool to delegate the work
3. Provide comprehensive context about the Rust/Actix-web stack
4. Let the subagent handle implementation autonomously

**Recommended Subagents for Backend:**
- `general-purpose`: API endpoint implementation, database integration
- `bug-hunter`: Rust compilation errors, runtime issues
- `debugger`: Test failures, performance problems
- `api-documenter`: OpenAPI spec, endpoint documentation
- `code-reviewer`: Rust code quality, SOLID principles
- `test-runner`: Cargo test execution and analysis
- `codex`: Architecture decisions, system design

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

### Test Organization
**✅ REQUIRED: Separate Test Files (NO inline #[cfg(test)] modules)**
```
qck-backend/
├── tests/                   # Integration tests
│   ├── postgres_test.rs    # Database pool tests
│   ├── redis_config_test.rs # Redis configuration tests
│   ├── redis_pool_test.rs  # Redis pool tests
│   └── api_endpoints_test.rs # API integration tests
├── .env.test               # Test environment (auto-loaded)
└── src/
    ├── lib.rs             # Library exports for testing
    └── modules/           # NO inline test modules
```

### Test Types
- **Unit Tests**: Individual functions, mocked dependencies
- **Integration Tests**: Full API endpoints with real connections
- **Performance Tests**: Load testing (1000+ ops/second)

### Test Execution
```bash
# Run all tests (auto-loads .env.test)
cargo test

# Run specific categories  
cargo test --test redis_pool_test
cargo test -- --nocapture
```

**Details**: See @.claude/memory/architecture/test-organization-best-practices.md

## Performance Guidelines

### Production Scaling for 1M Clicks/Day
- **Database**: 300 connections (12 avg/s, 100 peak/s)
- **Redis**: 150 connections (95%+ cache hit ratio)
- **Performance**: <50ms redirects, <1ms cache hits
- **Resources**: API(1GB), PostgreSQL(2GB), Redis(512MB)

**Details**: See @.claude/memory/architecture/production-scaling-1m-clicks.md

### General Performance
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

## 📊 Linear Organization

### Linear Projects Setup
Create these 3 main projects in Linear:
1. **QCK Core Platform** - Backend API + User Dashboard (tightly coupled)
2. **QCK Admin & Operations** - Admin panel + system operations
3. **QCK Marketing & Growth** - Marketing site + customer acquisition

### Issue Hierarchy
All items in Linear are Issues with parent-child relationships:
- **Epic Issues** (Label: `epic`) → Major feature groups
- **Feature Issues** (Label: `feature`) → Specific functionality
- **Task Issues** (Label: `task`) → Individual work items

### Label System (Multiple labels per issue)

**Available Linear Labels with IDs:**

**Platform Labels** (which component):
- `backend` - Backend/API work (PRIMARY for this project) - ID: `3d9d6f26-756e-48f6-918b-b54c47dccac1`
- `dashboard` - User dashboard/frontend - ID: `27849ae1-3d66-4ba3-be68-e11ccb021e4c`
- `admin` - Admin panel - ID: `9a590464-9eb3-44a6-90dc-a04567eaabe3`
- `marketing` - Marketing site - ID: `6fde7c66-dc69-4925-9b53-86ca9e95424d`

**Type Labels** (what kind of work):
- `api` - API endpoints (common for backend) - ID: `ff9a7ae0-453f-4c17-ba25-e8c398f7c875`
- `auth` - Authentication - ID: `50086f97-bfe0-42bb-9a00-64886326031c`
- `database` - Database work (common for backend) - ID: `9abb6b86-4fa1-4284-8370-1c57c2aa826a`
- `ui-ux` - User interface - ID: `ebb950d8-8bea-4c17-b38b-2ed841981d93`
- `security` - Security features - ID: `e5d152b5-1588-49f5-a9ef-40f9236d92aa`
- `performance` - Optimization - ID: `cfc5252e-6cfd-4dc5-ac3e-a6593cd71e98`
- `testing` - Tests - ID: `d797ba3c-a0d8-4fbd-8728-a00089ab84d2`
- `deployment` - DevOps - ID: `b54ced60-745e-4ae3-8abc-7a361b93752f`

**Hierarchy Labels**:
- `EPIC` - Top-level grouping - ID: `b69ab092-259a-4a43-a284-8c54ab582524`
- `Feature` - Mid-level functionality - ID: `d9c56e14-c8ba-4ded-8826-d6087a7378ab`
- `Task` - Individual work items - ID: `acde3a19-1570-4abb-83e9-7425ab0eaed1`

**Other Labels**:
- `Improvement` - Enhancements - ID: `103a1ed8-fc60-43de-842f-705b4f56b377`
- `Bug` - Bug fixes - ID: `f43c390a-3f87-4ca3-b47b-92148ee28a74`
- `tech debt` - Technical debt - ID: `91cfb438-cf41-4131-89ff-d2d8a89d5398`
- `Sub-Task` - Sub-tasks - ID: `e07e6bbe-33ff-4e9f-8280-a6f903729868`

**Label Group Rules**:
- Platform labels (`backend`, `dashboard`, `admin`, `marketing`) can be used together
- Type labels (`api`, `auth`, `database`, `ui-ux`, `security`, `performance`, `testing`, `deployment`) are in the same group - only one per issue
- Hierarchy labels (`EPIC`, `Feature`, `Task`) are in the same group - only one per issue

### Backend-Specific Examples

**Epic: "Authentication & Security"**
- Labels: `epic`, `backend`, `auth`, `security`, `critical`
- Children:
  - **Feature: "User Registration System"**
    - Labels: `feature`, `backend`, `api`, `auth`
    - Children:
      - **Task: "Build registration API endpoint"**
        - Labels: `task`, `backend`, `api`, `auth`
      - **Task: "Setup email verification"**
        - Labels: `task`, `backend`, `api`, `auth`

**Epic: "URL Shortening Engine"**
- Labels: `epic`, `backend`, `api`, `database`, `critical`
- Children:
  - **Feature: "Link CRUD Operations"**
    - Labels: `feature`, `backend`, `api`, `database`
    - Children:
      - **Task: "Implement Base62 encoding"**
        - Labels: `task`, `backend`, `api`
      - **Task: "Build link creation endpoint"**
        - Labels: `task`, `backend`, `api`, `database`

### Team Configuration
- **Linear Team ID**: `8557f533-649e-476c-9d20-efe571bfe69c`
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
- Use `backend` label for all backend-specific work

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
│   └── 2025-08-08-redis-connection-pool.md
├── tasks/             # Individual tasks completed
├── fixes/             # Bug fixes and issues resolved
├── architecture/      # Architecture decisions and changes
│   ├── 2025-08-08-test-organization-best-practices.md
│   └── 2025-08-08-production-scaling-1m-clicks.md
└── index.md          # Master index of all changes
```

**See @.claude/memory/index.md for complete development history**

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