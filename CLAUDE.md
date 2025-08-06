# QCK Backend - Project Memory

> Rust-based monolithic backend API for the QCK URL shortener platform

## Project Context

This is the core backend service for QCK, handling all API operations, database interactions, and business logic. Built with Rust and Actix-web for maximum performance and reliability.

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

When working on backend tasks:
- Check for existing Linear issues first
- Update issue status when starting work
- Link commits to Linear issues
- Mark as "waiting for review" when complete

## Memory Breadcrumbs

Remember to document all significant changes in @.claude/memory/:
- New API endpoints
- Database schema changes
- Performance optimizations
- Bug fixes
- Architecture decisions

---

*This file is specific to the qck-backend project*
*Always keep this documentation updated with changes*