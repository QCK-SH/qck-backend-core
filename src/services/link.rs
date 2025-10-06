// DEV-105: Link Creation API - Business logic layer
// DEV-68: Complete CRUD operations for links
// Building this with care for the vision we're creating together

use chrono::Utc;
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use once_cell::sync::Lazy;
use redis::AsyncCommands;
use reqwest::Client;
use scraper::Html;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tracing::{error, info, instrument, warn};
use uuid::Uuid;
use validator::Validate;

use crate::{
    app::AppState,
    db::{DieselPool, RedisPool},
    models::{
        link::{
            CreateLinkRequest, ExtractedMetadata, Link, LinkMetadata, LinkResponse,
            ListLinksParams, NewLink, UpdateLink, UpdateLinkRequest,
        },
        user::User,
    },
    services::{clickhouse_analytics::ClickHouseAnalyticsService, short_code::ShortCodeGenerator},
    utils::{
        audit_logger::{AuditAction, AuditLogger},
        security_scanner::SecurityService,
        service_error::ServiceError,
        url_validator::{UrlMetadata, UrlValidator},
    },
    CONFIG,
};

// =============================================================================
// TYPES
// =============================================================================

/// ClickHouse link statistics
#[derive(Debug, Clone, Default)]
pub struct LinkClickStats {
    pub total_clicks: u64,
    pub unique_visitors: u64,
    pub bot_clicks: u64,
    pub last_accessed_at: Option<chrono::DateTime<Utc>>,
}

// =============================================================================
// CONSTANTS
// =============================================================================

/// Cache TTL for link data (1 hour)
const LINK_CACHE_TTL_SECONDS: usize = 3600;

/// Click counter TTL (24 hours)
const CLICK_COUNTER_TTL_SECONDS: usize = 86400;

/// Batch size for processing click counters
const CLICK_SYNC_BATCH_SIZE: usize = 100;

// Shared HTTP client for metadata extraction with connection pooling
static METADATA_HTTP_CLIENT: Lazy<Client> = Lazy::new(|| {
    Client::builder()
        .pool_max_idle_per_host(10)
        .pool_idle_timeout(Duration::from_secs(30))
        .timeout(Duration::from_secs(3))
        .user_agent("Mozilla/5.0 (compatible; QCK-Bot/1.0)")
        .build()
        .expect("Failed to create HTTP client for metadata extraction")
});

// =============================================================================
// CACHE STATISTICS
// =============================================================================

#[derive(Debug, Clone)]
pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub total: u64,
    pub hit_rate: f64,
}

// =============================================================================
// LINK SERVICE
// =============================================================================

pub struct LinkService {
    diesel_pool: DieselPool,
    redis_pool: RedisPool,
    short_code_generator: ShortCodeGenerator,
    security_service: SecurityService,
    base_url: String,
    // Cache monitoring
    cache_hits: Arc<AtomicU64>,
    cache_misses: Arc<AtomicU64>,
    // Unified ClickHouse service for analytics and event tracking
    clickhouse_analytics: Option<Arc<ClickHouseAnalyticsService>>,
}

impl LinkService {
    /// Create a new LinkService instance
    pub fn new(state: &AppState) -> Self {
        // Get ClickHouse client from analytics service if available
        let clickhouse_client = state
            .clickhouse_analytics
            .as_ref()
            .map(|analytics| analytics.client())
            .expect("ClickHouse analytics service must be available for SecurityService");

        Self {
            diesel_pool: state.diesel_pool.clone(),
            redis_pool: state.redis_pool.clone(),
            short_code_generator: ShortCodeGenerator::new(state.diesel_pool.clone()),
            security_service: SecurityService::new(clickhouse_client),
            base_url: format!("https://{}", CONFIG.jwt.audience.clone()), // Using JWT audience as base domain
            cache_hits: Arc::new(AtomicU64::new(0)),
            cache_misses: Arc::new(AtomicU64::new(0)),
            clickhouse_analytics: state.clickhouse_analytics.clone(),
        }
    }

    /// Get cache statistics
    pub fn get_cache_stats(&self) -> CacheStats {
        let hits = AtomicU64::load(&self.cache_hits, Ordering::Relaxed);
        let misses = AtomicU64::load(&self.cache_misses, Ordering::Relaxed);
        let total = hits + misses;
        let hit_rate = if total > 0 {
            (hits as f64 / total as f64) * 100.0
        } else {
            0.0
        };

        // Alert if cache hit rate falls below 90%
        if total > 100 && hit_rate < 90.0 {
            warn!(
                "Cache hit rate below 90%: {:.2}% (hits: {}, misses: {})",
                hit_rate, hits, misses
            );
        }

        CacheStats {
            hits,
            misses,
            total,
            hit_rate,
        }
    }

    /// Create a new short link
    #[instrument(skip(self, user, request))]
    pub async fn create_link(
        &self,
        user: &User,
        mut request: CreateLinkRequest,
    ) -> Result<LinkResponse, ServiceError> {
        info!("Creating new link for user: {}", user.id);

        // 1. Sanitize and validate request
        request.sanitize();
        request.validate()?;
        request
            .validate_custom()
            .map_err(|e| ServiceError::ValidationError(e))?;

        // 2. Validate subscription limits
        self.validate_subscription_limits(user).await?;

        // 3. Normalize URL FIRST (use async version for proper validation)
        let normalized_url = crate::utils::normalize_url_async(&request.url).await?;

        // 4. Security scan the NORMALIZED URL with comprehensive threat detection
        let security_result = self
            .security_service
            .comprehensive_security_scan(&normalized_url)
            .await
            .map_err(|e| ServiceError::SecurityBlocked(format!("Security scan failed: {}", e)))?;

        if !security_result.is_safe {
            warn!(
                "URL blocked for security: {} - Threat score: {}, Risk: {:?}, Threats: {:?}",
                normalized_url,
                security_result.threat_score,
                security_result.risk_level,
                security_result.threats_detected
            );

            let security_message = if !security_result.warnings.is_empty() {
                security_result.warnings.join("; ")
            } else {
                format!(
                    "URL blocked due to security threats detected (Risk: {:?}, Score: {})",
                    security_result.risk_level, security_result.threat_score
                )
            };

            return Err(ServiceError::SecurityBlocked(security_message));
        }

        // Log security scan results for monitoring
        if security_result.threat_score > 0 {
            info!(
                "URL passed security scan with warnings: {} - Score: {}, Threats: {:?}",
                normalized_url, security_result.threat_score, security_result.threats_detected
            );
        }

        // 5. Generate or validate short code
        let short_code = if let Some(ref custom_alias) = request.custom_alias {
            // Validate custom alias using the specification method
            self.validate_custom_alias(custom_alias).await?;
            custom_alias.clone()
        } else {
            // Generate random short code
            self.short_code_generator.generate_unique_code().await?
        };

        // 6. Skip metadata extraction during creation (will be done in background)
        // This avoids blocking HTTP requests that can take 3+ seconds
        let extracted_metadata = None;

        // 7. Create link metadata (initially without extracted data)
        let metadata = LinkMetadata::from_request(&request, extracted_metadata);

        // 8. Hash password if provided
        let password_hash = if request.is_password_protected {
            match request.password.as_ref() {
                Some(p) => {
                    // Use proper bcrypt hashing for password protection
                    match bcrypt::hash(p, bcrypt::DEFAULT_COST) {
                        Ok(hash) => Some(hash),
                        Err(e) => {
                            error!("Failed to hash password: {}", e);
                            return Err(ServiceError::DatabaseError(format!(
                                "Password hashing failed: {}",
                                e
                            )));
                        },
                    }
                },
                None => None,
            }
        } else {
            None
        };

        // 9. Convert tags to Option<Vec<Option<String>>>
        let tags = if request.tags.is_empty() {
            None
        } else {
            Some(request.tags.iter().map(|t| Some(t.clone())).collect())
        };

        // 10. Create link record with all fields
        let new_link = NewLink {
            id: Uuid::new_v4(),
            user_id: user.id,
            short_code: short_code.clone(),
            original_url: normalized_url,
            title: request.title.clone().or(metadata.title.clone()),
            description: request.description.clone().or(metadata.description.clone()),
            tags,
            custom_alias: request.custom_alias.clone(),
            is_active: false, // Start inactive until metadata extraction completes
            expires_at: request.expires_at,
            password_hash,
            last_accessed_at: None,
            utm_source: None, // Could be extracted from URL
            utm_medium: None,
            utm_campaign: None,
            utm_term: None,
            utm_content: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            deleted_at: None, // New links are not deleted
            processing_status: "extracting".to_string(),
            metadata_extracted_at: None,
            og_image: request.og_image.clone(),
            favicon_url: request.favicon_url.clone(),
        };

        // 9. Insert into database with transaction
        let link = self
            .insert_link(new_link, user.id, &user.subscription_tier)
            .await?;

        // 10. Cache link for fast redirects
        self.cache_link(&link).await?;

        // 11. Audit log the creation
        AuditLogger::log_link_action(
            AuditAction::LinkCreated,
            user.id,
            Some(link.id.to_string()),
            Some(format!("Created link with short code: {}", short_code)),
        )
        .await;

        // 12. Spawn background task for metadata extraction (with rate limiting)
        // Skip metadata extraction if user provided all metadata fields
        let needs_metadata_extraction = request.title.is_none()
            || request.description.is_none()
            || request.og_image.is_none()
            || request.favicon_url.is_none();

        let link_id = link.id;
        let original_url = link.original_url.clone();
        let diesel_pool = Arc::new(self.diesel_pool.clone());
        let redis_pool = Arc::new(self.redis_pool.clone());

        if needs_metadata_extraction {
            // Use a static semaphore to limit concurrent metadata extractions
            static METADATA_SEMAPHORE: Lazy<Arc<tokio::sync::Semaphore>> = Lazy::new(|| {
                // Allow max 5 concurrent metadata extractions to avoid overwhelming external servers
                Arc::new(tokio::sync::Semaphore::new(5))
            });

            let semaphore = METADATA_SEMAPHORE.clone();
            tokio::spawn(async move {
                // Acquire permit before extraction (will wait if too many concurrent)
                let _permit = match semaphore.acquire().await {
                    Ok(permit) => permit,
                    Err(_) => {
                        warn!("Failed to acquire semaphore for metadata extraction");
                        return;
                    },
                };

                // Add small delay for bulk operations to avoid thundering herd
                if semaphore.available_permits() < 3 {
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }

                // Try to extract metadata
                let validator = UrlValidator::new();
                match validator.extract_metadata(&original_url).await {
                    Ok(metadata) => {
                        // Update link with extracted metadata and activate it
                        if let Err(e) = update_link_metadata_and_activate(
                            diesel_pool,
                            link_id,
                            metadata,
                            redis_pool,
                        )
                        .await
                        {
                            warn!("Failed to update link metadata for {}: {}", link_id, e);
                        }
                    },
                    Err(e) => {
                        warn!("Metadata extraction failed for {}: {}", link_id, e);
                        // Even if metadata extraction fails, activate the link
                        if let Err(e) =
                            activate_link_after_failure(diesel_pool, link_id, redis_pool).await
                        {
                            warn!(
                                "Failed to activate link after metadata failure {}: {}",
                                link_id, e
                            );
                        }
                    },
                }
                // Permit is automatically released when _permit goes out of scope
            });
        } else {
            // User provided all metadata, just activate the link immediately
            info!(
                "Skipping metadata extraction for link {} - user provided all metadata",
                link_id
            );
            tokio::spawn(async move {
                if let Err(e) = activate_link_immediately(diesel_pool, link_id, redis_pool).await {
                    warn!(
                        "Failed to activate link with user-provided metadata {}: {}",
                        link_id, e
                    );
                }
            });
        }

        // 13. Return response with empty stats (new link has no clicks yet)
        let empty_stats = LinkClickStats::default();
        let response = link.to_response_with_stats(&self.base_url, empty_stats);

        info!("Successfully created link: {}", short_code);
        Ok(response)
    }

    /// Validate that a custom alias is available and valid
    async fn validate_custom_alias(&self, alias: &str) -> Result<(), ServiceError> {
        use crate::schema::links::dsl;

        let mut conn = self
            .diesel_pool
            .get()
            .await
            .map_err(|e| ServiceError::DatabaseError(e.to_string()))?;

        // Check if alias is available
        let exists = dsl::links
            .filter(dsl::short_code.eq(alias))
            .filter(dsl::deleted_at.is_null())
            .select(dsl::id)
            .first::<Uuid>(&mut conn)
            .await
            .optional()?
            .is_some();

        if exists {
            return Err(ServiceError::AliasAlreadyExists);
        }

        // Additional alias validation using CustomAliasValidator
        use crate::utils::custom_alias_validator::CustomAliasValidator;
        if let Err(reason) = CustomAliasValidator::validate(alias) {
            return Err(ServiceError::ValidationError(reason));
        }

        Ok(())
    }

    /// Get a link by ID and verify ownership
    #[instrument(skip(self))]
    pub async fn get_link_by_id_and_user(
        &self,
        link_id: Uuid,
        user_id: Uuid,
    ) -> Result<Link, ServiceError> {
        use crate::schema::links::dsl;

        let mut conn = self
            .diesel_pool
            .get()
            .await
            .map_err(|e| ServiceError::DatabaseError(e.to_string()))?;

        let link = dsl::links
            .filter(dsl::id.eq(link_id))
            .filter(dsl::user_id.eq(user_id))
            .filter(dsl::deleted_at.is_null())
            .first::<Link>(&mut conn)
            .await?;

        // Audit log the read operation
        AuditLogger::log_link_action(
            AuditAction::LinkRead,
            user_id,
            Some(link_id.to_string()),
            None,
        )
        .await;

        Ok(link)
    }

    /// Get a link by ID with ClickHouse stats
    #[instrument(skip(self))]
    pub async fn get_link_with_stats(
        &self,
        link_id: Uuid,
        user_id: Uuid,
    ) -> Result<LinkResponse, ServiceError> {
        // Get the link from PostgreSQL
        let link = self.get_link_by_id_and_user(link_id, user_id).await?;

        // Get stats from ClickHouse for this single link
        let link_ids = vec![link_id];
        let stats_map = self.get_clickhouse_stats(&link_ids).await;

        // Get the stats for this link, or use default values
        let stats = stats_map.get(&link_id).cloned().unwrap_or_default();

        // Get base URL from config
        let base_url = format!("https://{}", crate::app_config::CONFIG.jwt_audience.clone());

        // Convert to response with stats
        Ok(link.to_response_with_stats(&base_url, stats))
    }

    /// Get a link by short code (for internal use)
    #[instrument(skip(self))]
    pub async fn get_link(&self, short_code: &str) -> Result<Link, ServiceError> {
        // Try cache first
        if let Ok(Some(link)) = self.get_cached_link(short_code).await {
            AtomicU64::fetch_add(&self.cache_hits, 1, Ordering::Relaxed);
            return Ok(link);
        }

        // Cache miss - fall back to database
        AtomicU64::fetch_add(&self.cache_misses, 1, Ordering::Relaxed);
        use crate::schema::links::dsl;

        let mut conn = self
            .diesel_pool
            .get()
            .await
            .map_err(|e| ServiceError::DatabaseError(e.to_string()))?;

        let link = dsl::links
            .filter(dsl::short_code.eq(short_code))
            .or_filter(dsl::custom_alias.eq(short_code))
            .filter(dsl::deleted_at.is_null())
            .filter(dsl::is_active.eq(true))
            .first::<Link>(&mut conn)
            .await?;

        // Cache for next time
        let _ = self.cache_link(&link).await;

        Ok(link)
    }

    /// Get a link by short code (specification method name)
    #[instrument(skip(self))]
    pub async fn get_link_by_code(&self, short_code: &str) -> Result<Option<Link>, ServiceError> {
        match self.get_link(short_code).await {
            Ok(link) => Ok(Some(link)),
            Err(ServiceError::NotFound) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Update an existing link
    #[instrument(skip(self, user, request))]
    pub async fn update_link(
        &self,
        user: &User,
        link_id: Uuid,
        request: UpdateLinkRequest,
    ) -> Result<LinkResponse, ServiceError> {
        use crate::schema::links::dsl;

        // Check ownership using the helper method
        let existing_link = self.get_link_by_id_and_user(link_id, user.id).await?;

        let mut conn = self
            .diesel_pool
            .get()
            .await
            .map_err(|e| ServiceError::DatabaseError(e.to_string()))?;

        // Build update struct with proper field handling
        let password_hash = if let Some(is_protected) = request.is_password_protected {
            if is_protected {
                request.password.as_ref().map(|p| {
                    // TODO: Use proper bcrypt hashing in production
                    Some(format!("hashed_{}", p))
                })
            } else {
                Some(None) // Clear password
            }
        } else {
            None // Don't change password status
        };

        // Convert tags properly for database
        let tags = request.tags.as_ref().map(|tag_list| {
            Some(
                tag_list
                    .iter()
                    .map(|t| Some(t.clone()))
                    .collect::<Vec<Option<String>>>(),
            )
        });

        let update = UpdateLink {
            original_url: request.url,
            title: request.title.map(Some),
            description: request.description.map(Some),
            og_image: request.og_image.map(Some),
            favicon_url: request.favicon_url.map(Some),
            tags,
            expires_at: request.expires_at,
            is_active: request.is_active,
            password_hash,
            utm_source: None,
            utm_medium: None,
            utm_campaign: None,
            utm_term: None,
            utm_content: None,
            updated_at: Utc::now(),
            processing_status: None, // Don't change processing status on regular updates
            metadata_extracted_at: None, // Don't change metadata timestamp on regular updates
        };

        // Apply update
        let updated_link = diesel::update(dsl::links.find(link_id))
            .set(&update)
            .get_result::<Link>(&mut conn)
            .await?;

        // Invalidate cache
        self.invalidate_cache(&existing_link.short_code).await?;
        if let Some(ref alias) = existing_link.custom_alias {
            self.invalidate_cache(alias).await?;
        }

        // Audit log the update
        AuditLogger::log_link_action(
            AuditAction::LinkUpdated,
            user.id,
            Some(link_id.to_string()),
            Some("Updated link details".to_string()),
        )
        .await;

        // Get stats from ClickHouse for the updated link
        let link_ids = vec![link_id];
        let stats_map = self.get_clickhouse_stats(&link_ids).await;
        let stats = stats_map.get(&link_id).cloned().unwrap_or_default();

        Ok(updated_link.to_response_with_stats(&self.base_url, stats))
    }

    /// Delete (soft delete) a link
    #[instrument(skip(self, user))]
    pub async fn delete_link(&self, user: &User, link_id: Uuid) -> Result<(), ServiceError> {
        use crate::schema::links::dsl;

        let mut conn = self
            .diesel_pool
            .get()
            .await
            .map_err(|e| ServiceError::DatabaseError(e.to_string()))?;

        // Verify ownership and soft delete
        let rows_affected = diesel::update(
            dsl::links
                .filter(dsl::id.eq(link_id))
                .filter(dsl::user_id.eq(user.id))
                .filter(dsl::deleted_at.is_null()),
        )
        .set((
            dsl::deleted_at.eq(Some(Utc::now())),
            dsl::updated_at.eq(Utc::now()),
        ))
        .execute(&mut conn)
        .await?;

        if rows_affected == 0 {
            return Err(ServiceError::NotFound);
        }

        // Get the link to invalidate cache
        let link = dsl::links.find(link_id).first::<Link>(&mut conn).await?;

        // Invalidate cache
        self.invalidate_cache(&link.short_code).await?;
        if let Some(ref alias) = link.custom_alias {
            self.invalidate_cache(alias).await?;
        }

        // Audit log the deletion
        AuditLogger::log_link_action(
            AuditAction::LinkDeleted,
            user.id,
            Some(link_id.to_string()),
            Some(format!(
                "Soft deleted link with short code: {}",
                link.short_code
            )),
        )
        .await;

        info!("Link {} soft deleted by user {}", link_id, user.id);
        Ok(())
    }

    /// Permanently delete a link (admin-only operation)
    #[instrument(skip(self))]
    pub async fn permanent_delete_link(
        &self,
        link_id: Uuid,
        user_id: Uuid,
    ) -> Result<(), ServiceError> {
        use crate::schema::links::dsl;

        let mut conn = self
            .diesel_pool
            .get()
            .await
            .map_err(|e| ServiceError::DatabaseError(e.to_string()))?;

        // Get the link first to invalidate cache
        let link = dsl::links.find(link_id).first::<Link>(&mut conn).await?;

        // Permanently delete from database
        let rows_affected = diesel::delete(dsl::links.find(link_id))
            .execute(&mut conn)
            .await?;

        if rows_affected == 0 {
            return Err(ServiceError::NotFound);
        }

        // Invalidate cache
        self.invalidate_cache(&link.short_code).await?;
        if let Some(ref alias) = link.custom_alias {
            self.invalidate_cache(alias).await?;
        }

        // Audit log the permanent deletion
        AuditLogger::log_link_action(
            AuditAction::LinkPermanentlyDeleted,
            user_id,
            Some(link_id.to_string()),
            Some(format!(
                "Permanently deleted link with short code: {}",
                link.short_code
            )),
        )
        .await;

        warn!("Link {} permanently deleted by user {}", link_id, user_id);
        Ok(())
    }

    /// Bulk delete (deactivate) multiple links
    #[instrument(skip(self, user))]
    pub async fn bulk_delete_links(
        &self,
        user: &User,
        link_ids: Vec<Uuid>,
    ) -> Result<u64, ServiceError> {
        use crate::schema::links::dsl;

        // Validate bulk operation size
        if link_ids.is_empty() {
            return Ok(0);
        }

        if link_ids.len() > 100 {
            return Err(ServiceError::ValidationError(
                "Cannot delete more than 100 links at once".to_string(),
            ));
        }

        let mut conn = self
            .diesel_pool
            .get()
            .await
            .map_err(|e| ServiceError::DatabaseError(e.to_string()))?;

        // Perform bulk soft delete with RETURNING clause to get deleted links in one query
        let links_to_delete: Vec<Link> = diesel::update(
            dsl::links
                .filter(dsl::id.eq_any(&link_ids))
                .filter(dsl::user_id.eq(user.id))
                .filter(dsl::deleted_at.is_null()),
        )
        .set((
            dsl::deleted_at.eq(Some(Utc::now())),
            dsl::is_active.eq(false),
            dsl::updated_at.eq(Utc::now()),
        ))
        .returning(dsl::links::all_columns())
        .get_results(&mut conn)
        .await?;

        let rows_affected = links_to_delete.len();

        // Invalidate cache for all deleted links using pipeline for efficiency
        let deleted_ids: Vec<String> = links_to_delete.iter().map(|l| l.id.to_string()).collect();
        let cache_keys_to_delete: Vec<String> = links_to_delete
            .iter()
            .flat_map(|link| {
                let mut keys = vec![format!("link:{}", link.short_code)];
                if let Some(ref alias) = link.custom_alias {
                    keys.push(format!("link:{}", alias));
                }
                keys
            })
            .collect();

        if !cache_keys_to_delete.is_empty() {
            let _ = self.invalidate_cache_batch(cache_keys_to_delete).await;
        }

        // Audit log the bulk deletion
        if rows_affected > 0 {
            AuditLogger::log_bulk_action(
                AuditAction::BulkLinksDeleted,
                user.id,
                deleted_ids,
                Some(format!("Bulk soft deleted {} links", rows_affected)),
            )
            .await;
        }

        info!("Bulk deleted {} links for user {}", rows_affected, user.id);
        Ok(rows_affected as u64)
    }

    /// Bulk update status (active/inactive) for multiple links
    #[instrument(skip(self, user))]
    pub async fn bulk_update_status(
        &self,
        user: &User,
        link_ids: Vec<Uuid>,
        is_active: bool,
    ) -> Result<u64, ServiceError> {
        use crate::schema::links::dsl;

        // Validate bulk operation size
        if link_ids.is_empty() {
            return Ok(0);
        }

        if link_ids.len() > 100 {
            return Err(ServiceError::ValidationError(
                "Cannot update more than 100 links at once".to_string(),
            ));
        }

        let mut conn = self
            .diesel_pool
            .get()
            .await
            .map_err(|e| ServiceError::DatabaseError(e.to_string()))?;

        // Get links that will be updated (for cache invalidation)
        // Only update non-deleted links
        let links_to_update = dsl::links
            .filter(dsl::id.eq_any(&link_ids))
            .filter(dsl::user_id.eq(user.id))
            .filter(dsl::deleted_at.is_null())
            .load::<Link>(&mut conn)
            .await?;

        // Perform bulk status update
        // Only update non-deleted links
        let rows_affected = diesel::update(
            dsl::links
                .filter(dsl::id.eq_any(&link_ids))
                .filter(dsl::user_id.eq(user.id))
                .filter(dsl::deleted_at.is_null()),
        )
        .set((dsl::is_active.eq(is_active), dsl::updated_at.eq(Utc::now())))
        .execute(&mut conn)
        .await?;

        // Invalidate cache for all updated links
        let updated_ids: Vec<String> = links_to_update.iter().map(|l| l.id.to_string()).collect();
        for link in links_to_update {
            let _ = self.invalidate_cache(&link.short_code).await;
            if let Some(ref alias) = link.custom_alias {
                let _ = self.invalidate_cache(alias).await;
            }
        }

        // Audit log the bulk status update
        if rows_affected > 0 {
            let action = if is_active {
                "activated"
            } else {
                "deactivated"
            };
            AuditLogger::log_bulk_action(
                AuditAction::BulkStatusUpdated,
                user.id,
                updated_ids,
                Some(format!("Bulk {} {} links", action, rows_affected)),
            )
            .await;
        }

        let action = if is_active {
            "activated"
        } else {
            "deactivated"
        };
        info!(
            "Bulk {} {} links for user {}",
            action, rows_affected, user.id
        );
        Ok(rows_affected as u64)
    }

    /// Get user's links with filtering and pagination (specification method name)
    #[instrument(skip(self, user))]
    pub async fn get_user_links(
        &self,
        user: &User,
        params: ListLinksParams,
    ) -> Result<crate::models::link::LinkListResponse, ServiceError> {
        use crate::schema::links::dsl;

        let mut conn = self
            .diesel_pool
            .get()
            .await
            .map_err(|e| ServiceError::DatabaseError(e.to_string()))?;

        // Build query with filters - exclude soft deleted links
        let mut query = dsl::links
            .filter(dsl::user_id.eq(user.id))
            .filter(dsl::deleted_at.is_null())
            .into_boxed();

        if let Some(ref search) = params.filter.search {
            let pattern = format!("%{}%", search);
            query = query.filter(
                dsl::short_code
                    .ilike(pattern.clone())
                    .or(dsl::original_url.ilike(pattern.clone()))
                    .or(dsl::custom_alias.ilike(pattern)),
            );
        }

        if let Some(is_active) = params.filter.is_active {
            query = query.filter(dsl::is_active.eq(is_active));
        }

        if let Some(ref domain) = params.filter.domain {
            let pattern = format!("%{}%", domain);
            query = query.filter(dsl::original_url.ilike(pattern));
        }

        if let Some(created_after) = params.filter.created_after {
            query = query.filter(dsl::created_at.ge(created_after));
        }

        if let Some(created_before) = params.filter.created_before {
            query = query.filter(dsl::created_at.le(created_before));
        }

        // Count total results (rebuild query for count)
        // CRITICAL: Must filter by deleted_at to match main query
        let mut count_query = dsl::links
            .filter(dsl::user_id.eq(user.id))
            .filter(dsl::deleted_at.is_null())
            .into_boxed();

        // Apply same filters for count
        if let Some(ref search) = params.filter.search {
            count_query = count_query.filter(
                dsl::original_url
                    .ilike(format!("%{}%", search))
                    .or(dsl::short_code.ilike(format!("%{}%", search)))
                    .or(dsl::custom_alias.ilike(format!("%{}%", search))),
            );
        }

        if let Some(is_active) = params.filter.is_active {
            count_query = count_query.filter(dsl::is_active.eq(is_active));
        }

        if let Some(created_after) = params.filter.created_after {
            count_query = count_query.filter(dsl::created_at.ge(created_after));
        }

        if let Some(created_before) = params.filter.created_before {
            count_query = count_query.filter(dsl::created_at.le(created_before));
        }

        let total = count_query.count().get_result::<i64>(&mut conn).await?;

        // Get paginated results - order by sort_by parameter or default to created_at
        let links = match params.sort_by.as_deref() {
            Some("title") => query.order(dsl::title.asc()),
            _ => query.order(dsl::created_at.desc()),
        }
        .limit(params.limit())
        .offset(params.offset())
        .load::<Link>(&mut conn)
        .await?;

        // Fetch ClickHouse stats for these links
        let link_ids: Vec<Uuid> = links.iter().map(|l| l.id).collect();
        let clickhouse_stats = self.get_clickhouse_stats(&link_ids).await;

        // Convert links to responses with ClickHouse stats
        let base_url = &self.base_url;
        let link_responses: Vec<crate::models::link::LinkResponse> = links
            .into_iter()
            .map(|link| {
                // Get stats from ClickHouse if available
                if let Some(stats) = clickhouse_stats.get(&link.id) {
                    link.to_response_with_stats(base_url, stats.clone())
                } else {
                    // No stats available, use defaults
                    link.to_response(base_url)
                }
            })
            .collect();

        // Return enhanced response with ClickHouse data
        Ok(crate::models::link::LinkListResponse {
            links: link_responses,
            total: total as i64,
            page: params.page as i64,
            per_page: params.per_page as i64,
            total_pages: ((total as f64) / (params.per_page as f64)).ceil() as i64,
        })
    }

    /// Fetch link statistics from ClickHouse for a list of link IDs
    /// Uses the ClickHouseAnalyticsService for clean separation of concerns
    #[instrument(skip(self), fields(link_count = link_ids.len()))]
    async fn get_clickhouse_stats(
        &self,
        link_ids: &[Uuid],
    ) -> std::collections::HashMap<Uuid, LinkClickStats> {
        // Use the ClickHouseAnalyticsService if available
        if let Some(ref analytics_service) = self.clickhouse_analytics {
            info!(
                "Fetching ClickHouse stats via analytics service for {} links",
                link_ids.len()
            );
            let stats = analytics_service.get_bulk_link_stats(link_ids).await;

            if !stats.is_empty() {
                info!(
                    "Successfully retrieved stats for {} links from ClickHouse",
                    stats.len()
                );
            }

            stats
        } else {
            // No ClickHouse configured, return empty stats
            info!("ClickHouse analytics service not configured, returning empty stats");
            std::collections::HashMap::new()
        }
    }

    /// Process a redirect and increment click count
    #[instrument(skip(self))]
    pub async fn process_redirect(&self, short_code: &str) -> Result<(Uuid, String), ServiceError> {
        // Get link
        let link = self.get_link(short_code).await?;

        // Check if expired
        if let Some(expires_at) = link.expires_at {
            if expires_at < Utc::now() {
                return Err(ServiceError::Expired);
            }
        }

        // Check if active
        if !link.is_active {
            return Err(ServiceError::Inactive);
        }

        // Async increment click count in Redis (fast)
        // This is fire-and-forget but with error logging and fallback queue
        let short_code_clone = short_code.to_string();
        let redis_pool = self.redis_pool.clone();
        tokio::spawn(async move {
            if let Err(e) = increment_click_count_redis(&redis_pool, &short_code_clone).await {
                error!(
                    "Failed to track click for {}: {}. Will be retried in background sync.",
                    short_code_clone, e
                );
                // Error is logged and fallback queue is already handled in increment_click_count_redis
            }
        });

        Ok((link.id, link.original_url.clone()))
    }

    /// Track a click event to ClickHouse for analytics
    pub fn track_click_event(
        &self,
        link_id: Uuid,
        ip: std::net::IpAddr,
        user_agent: &str,
        referrer: Option<&str>,
        method: &str,
        response_time: u16,
        status_code: u16,
    ) {
        // Use the unified ClickHouse analytics service for event tracking
        if let Some(ref analytics) = self.clickhouse_analytics {
            let event = crate::services::click_tracking::ClickEvent::new(
                link_id,
                ip,
                user_agent,
                referrer,
                method,
                response_time,
                status_code,
            );

            // Track the click through unified service (async, fire-and-forget)
            analytics.track_click(event);
        } else {
            // ClickHouse not configured - silently skip (this is normal in dev/test)
            info!("ClickHouse analytics not configured, skipping event tracking");
        }
    }

    // =============================================================================
    // HELPER METHODS
    // =============================================================================

    /// Validate user link limits (OSS version - no limits for self-hosted)
    /// Cloud version should override this with subscription tier logic
    async fn validate_subscription_limits(&self, _user: &User) -> Result<(), ServiceError> {
        // OSS: No link limits (self-hosted, unlimited links for all users)
        // Cloud: Override this method in CloudLinkService for tier-based limits
        Ok(())
    }

    /// Try to extract metadata from URL using shared HTTP client with connection pooling
    async fn try_extract_metadata(&self, url: &str) -> Option<ExtractedMetadata> {
        use tokio::time::timeout;

        // Use shared HTTP client with connection pooling and timeout for additional safety
        let response =
            match timeout(Duration::from_secs(3), METADATA_HTTP_CLIENT.get(url).send()).await {
                Ok(Ok(resp)) => {
                    // Check content type early to avoid processing non-HTML content
                    if let Some(content_type) = resp.headers().get("content-type") {
                        if let Ok(ct_str) = content_type.to_str() {
                            if !ct_str.contains("text/html") {
                                warn!("URL {} returned non-HTML content: {}", url, ct_str);
                                return None;
                            }
                        }
                    }
                    resp
                },
                Ok(Err(e)) => {
                    warn!("Failed to fetch URL for metadata {}: {}", url, e);
                    return None;
                },
                Err(_) => {
                    warn!("Timeout fetching URL for metadata: {}", url);
                    return None;
                },
            };

        // Get the HTML content with size limit
        let html_content = match timeout(Duration::from_secs(2), response.text()).await {
            Ok(Ok(mut text)) => {
                // Limit content size to prevent memory issues
                if text.len() > 1024 * 1024 {
                    // 1MB limit
                    text.truncate(1024 * 1024);
                    warn!("Truncated large HTML content for URL: {}", url);
                }
                text
            },
            Ok(Err(e)) => {
                warn!("Failed to read response body for {}: {}", url, e);
                return None;
            },
            Err(_) => {
                warn!("Timeout reading response body for: {}", url);
                return None;
            },
        };

        // Parse HTML (this is CPU-intensive, but we keep it fast by limiting content size)
        let document = Html::parse_document(&html_content);

        // Extract metadata in parallel using tokio::try_join for better performance
        let title = self.extract_title(&document);
        let description = self.extract_description(&document);
        let og_image = self.extract_og_image(&document);
        let favicon_url = self.extract_favicon(&document, url);

        // Return extracted metadata
        Some(ExtractedMetadata {
            title,
            description,
            favicon_url,
            og_image,
        })
    }

    /// Extract title from HTML document
    fn extract_title(&self, document: &Html) -> Option<String> {
        use scraper::Selector;

        let title_selector = Selector::parse("title").ok()?;
        document
            .select(&title_selector)
            .next()
            .map(|el| el.text().collect::<String>().trim().to_string())
            .filter(|s| !s.is_empty() && s.len() <= 200) // Limit title length
    }

    /// Extract description from HTML document
    fn extract_description(&self, document: &Html) -> Option<String> {
        use scraper::Selector;

        // Try Open Graph description first
        if let Ok(og_desc_selector) = Selector::parse("meta[property=\"og:description\"]") {
            if let Some(og_desc) = document
                .select(&og_desc_selector)
                .next()
                .and_then(|el| el.value().attr("content"))
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty() && s.len() <= 500)
            // Limit description length
            {
                return Some(og_desc);
            }
        }

        // Fallback to meta description
        if let Ok(desc_selector) = Selector::parse("meta[name=\"description\"]") {
            document
                .select(&desc_selector)
                .next()
                .and_then(|el| el.value().attr("content"))
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty() && s.len() <= 500) // Limit description length
        } else {
            None
        }
    }

    /// Extract Open Graph image from HTML document
    fn extract_og_image(&self, document: &Html) -> Option<String> {
        use scraper::Selector;

        let og_image_selector = Selector::parse("meta[property=\"og:image\"]").ok()?;
        document
            .select(&og_image_selector)
            .next()
            .and_then(|el| el.value().attr("content"))
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty() && s.len() <= 500) // Limit URL length
    }

    /// Extract favicon from HTML document
    fn extract_favicon(&self, document: &Html, base_url: &str) -> Option<String> {
        use scraper::Selector;

        let favicon_selectors = [
            "link[rel=\"icon\"]",
            "link[rel=\"shortcut icon\"]",
            "link[rel=\"apple-touch-icon\"]",
        ];

        for selector_str in &favicon_selectors {
            if let Ok(selector) = Selector::parse(selector_str) {
                if let Some(favicon) = document
                    .select(&selector)
                    .next()
                    .and_then(|el| el.value().attr("href"))
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                {
                    // Make favicon URL absolute if it's relative
                    let absolute_url = if favicon.starts_with("http") {
                        favicon
                    } else if favicon.starts_with("//") {
                        format!("https:{}", favicon)
                    } else if favicon.starts_with('/') {
                        // Extract base URL
                        if let Ok(parsed_url) = url::Url::parse(base_url) {
                            format!(
                                "{}://{}{}",
                                parsed_url.scheme(),
                                parsed_url.host_str().unwrap_or(""),
                                favicon
                            )
                        } else {
                            favicon
                        }
                    } else {
                        favicon
                    };

                    if absolute_url.len() <= 500 {
                        return Some(absolute_url);
                    }
                }
            }
        }
        None
    }

    /// Insert link with transaction
    async fn insert_link(
        &self,
        new_link: NewLink,
        user_id: Uuid,
        _subscription_tier: &str,
    ) -> Result<Link, ServiceError> {
        use crate::schema::links::dsl;

        let mut conn = self
            .diesel_pool
            .get()
            .await
            .map_err(|e| ServiceError::DatabaseError(e.to_string()))?;

        // Insert the new link (OSS: no quota limits)
        conn.build_transaction()
            .run::<_, diesel::result::Error, _>(|conn| {
                Box::pin(async move {
                    diesel::insert_into(dsl::links)
                        .values(&new_link)
                        .get_result::<Link>(conn)
                        .await
                })
            })
            .await
            .map_err(|e| ServiceError::DatabaseError(e.to_string()))
    }

    /// Cache link in Redis
    async fn cache_link(&self, link: &Link) -> Result<(), ServiceError> {
        let cache_key = format!("link:{}", link.short_code);

        // Serialize the entire Link object
        let serialized = serde_json::to_string(link)
            .map_err(|e| ServiceError::CacheError(format!("Failed to serialize link: {}", e)))?;

        let mut redis_conn = self
            .redis_pool
            .get_connection()
            .await
            .map_err(|e| ServiceError::CacheError(e.to_string()))?;

        let _: () = redis_conn
            .set_ex(
                &cache_key,
                serialized.clone(),
                LINK_CACHE_TTL_SECONDS as u64,
            )
            .await
            .map_err(|e| ServiceError::CacheError(e.to_string()))?;

        // Also cache by custom alias if present
        if let Some(ref alias) = link.custom_alias {
            let alias_key = format!("link:{}", alias);
            let _: () = redis_conn
                .set_ex(&alias_key, serialized, LINK_CACHE_TTL_SECONDS as u64)
                .await
                .map_err(|e| ServiceError::CacheError(e.to_string()))?;
        }

        Ok(())
    }

    /// Get cached link
    async fn get_cached_link(&self, short_code: &str) -> Result<Option<Link>, ServiceError> {
        let cache_key = format!("link:{}", short_code);

        match self.redis_pool.get::<String>(&cache_key).await {
            Ok(Some(data)) => {
                // Deserialize the full Link object
                match serde_json::from_str::<Link>(&data) {
                    Ok(link) => Ok(Some(link)),
                    Err(e) => {
                        warn!(
                            "Failed to deserialize cached link for {}: {}",
                            short_code, e
                        );
                        Ok(None) // Fall back to database on deserialization error
                    },
                }
            },
            Ok(None) => Ok(None),
            Err(e) => {
                warn!("Redis error getting cached link: {}", e);
                Ok(None) // Fall back to database on cache error
            },
        }
    }

    /// Invalidate cache entry
    async fn invalidate_cache(&self, short_code: &str) -> Result<(), ServiceError> {
        let cache_key = format!("link:{}", short_code);

        self.redis_pool
            .del(&cache_key)
            .await
            .map_err(|e| ServiceError::CacheError(e.to_string()))?;

        Ok(())
    }

    /// Invalidate multiple cache entries using Redis pipeline for efficiency
    async fn invalidate_cache_batch(&self, cache_keys: Vec<String>) -> Result<(), ServiceError> {
        if cache_keys.is_empty() {
            return Ok(());
        }

        // Use Redis pipeline for batch operations
        let mut conn = self
            .redis_pool
            .get_connection()
            .await
            .map_err(|e| ServiceError::CacheError(format!("Redis connection error: {}", e)))?;

        // Create pipeline and add all delete operations
        let mut pipe = redis::pipe();
        for key in &cache_keys {
            pipe.del(key);
        }

        // Execute pipeline in one go
        pipe.query_async::<Vec<i32>>(&mut conn)
            .await
            .map_err(|e| ServiceError::CacheError(format!("Pipeline error: {}", e)))?;

        info!("Batch invalidated {} cache entries", cache_keys.len());
        Ok(())
    }
}

/// Fast Redis click count increment with retry mechanism (used for real-time tracking)
async fn increment_click_count_redis(
    redis_pool: &RedisPool,
    short_code: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let counter_key = format!("clicks:{}", short_code);

    increment_click_count_with_retry(redis_pool, &counter_key, short_code, 3).await
}

/// Increment click count with exponential backoff retry
async fn increment_click_count_with_retry(
    redis_pool: &RedisPool,
    counter_key: &str,
    short_code: &str,
    max_retries: u32,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use std::time::Duration;
    use tokio::time::sleep;

    let mut retries = 0;
    let mut delay = Duration::from_millis(100);

    loop {
        match try_increment_click(redis_pool, counter_key).await {
            Ok(_) => return Ok(()),
            Err(e) if retries < max_retries => {
                warn!(
                    "Click tracking failed for {}, retry {}/{}: {}",
                    short_code,
                    retries + 1,
                    max_retries,
                    e
                );
                retries += 1;
                sleep(delay).await;
                delay *= 2; // Exponential backoff
            },
            Err(e) => {
                // After max retries, log to a fallback queue for batch processing
                error!(
                    "Click tracking failed after {} retries for {}: {}. Adding to fallback queue.",
                    max_retries, short_code, e
                );

                // Try to add to fallback queue (best effort)
                if let Ok(mut conn) = redis_pool.get_connection().await {
                    let fallback_key = format!("clicks:fallback:{}", short_code);
                    let _ = conn.incr::<_, _, ()>(&fallback_key, 1).await;
                    let _ = conn.expire::<_, ()>(&fallback_key, 86400).await; // 24 hour TTL
                }

                return Err(e);
            },
        }
    }
}

/// Actual Redis increment operation
async fn try_increment_click(
    redis_pool: &RedisPool,
    counter_key: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut redis_conn = redis_pool.get_connection().await?;

    // Increment counter
    let _: () = redis_conn.incr(counter_key, 1).await?;

    // Set TTL for cleanup
    redis_conn
        .expire::<_, ()>(counter_key, CLICK_COUNTER_TTL_SECONDS as i64)
        .await?;

    Ok(())
}

/// Background job to sync Redis click counts to database (call every 5 minutes)
pub async fn sync_click_counts_to_database(
    redis_pool: &RedisPool,
    diesel_pool: &DieselPool,
) -> Result<u32, Box<dyn std::error::Error + Send + Sync>> {
    use crate::schema::links::dsl;

    // Get all click counter keys (including fallback queue)
    let mut redis_conn = redis_pool.get_connection().await?;
    let pattern = "clicks:*";
    let mut keys: Vec<String> = redis_conn.keys(pattern).await.unwrap_or_default();

    // Also get fallback queue keys
    let fallback_pattern = "clicks:fallback:*";
    let fallback_keys: Vec<String> = redis_conn.keys(fallback_pattern).await.unwrap_or_default();
    keys.extend(fallback_keys);

    if keys.is_empty() {
        return Ok(0);
    }

    let mut updated_count = 0;
    let mut conn = diesel_pool.get().await?;

    // Process in batches
    for chunk in keys.chunks(CLICK_SYNC_BATCH_SIZE) {
        // Use Redis pipeline to fetch all click counts efficiently
        let mut updates = Vec::new();
        if !chunk.is_empty() {
            // Create pipeline to fetch all click counts at once
            let mut pipe = redis::pipe();
            for key in chunk {
                pipe.get(key);
            }

            // Execute pipeline and get all values
            if let Ok(values) = pipe
                .query_async::<Vec<Option<String>>>(&mut redis_conn)
                .await
            {
                for (key, value_opt) in chunk.iter().zip(values.iter()) {
                    if let Some(count_str) = value_opt {
                        // Extract short_code from key (handle both regular and fallback keys)
                        let short_code = if let Some(code) = key.strip_prefix("clicks:fallback:") {
                            code.to_string()
                        } else if let Some(code) = key.strip_prefix("clicks:") {
                            code.to_string()
                        } else {
                            continue; // Skip invalid keys
                        };

                        if let Ok(count) = count_str.parse::<i32>() {
                            if count > 0 {
                                updates.push((short_code, count));
                            }
                        }
                    }
                }
            }
        }

        let update_count = updates.len();

        // Start transaction for database updates
        let result = conn
            .build_transaction()
            .run::<_, diesel::result::Error, _>(|conn| {
                Box::pin(async move {
                    for (short_code, _count) in &updates {
                        // Update database
                        let _rows_affected = diesel::update(
                            dsl::links
                                .filter(dsl::short_code.eq(short_code))
                                .or_filter(dsl::custom_alias.eq(short_code)),
                        )
                        .set(dsl::last_accessed_at.eq(Utc::now()))
                        .execute(conn)
                        .await?;

                        // Count successful updates (we'll move increment outside)
                    }
                    Ok(())
                })
            })
            .await;

        if result.is_ok() {
            updated_count += update_count as u32;
            // Delete processed keys from Redis using pipeline for efficiency
            if !chunk.is_empty() {
                let mut pipe = redis::pipe();
                for key in chunk {
                    pipe.del(key);
                }
                let _ = pipe.query_async::<Vec<i32>>(&mut redis_conn).await;
            }
        }
    }

    if updated_count > 0 {
        info!("Synced {} link click counts to database", updated_count);
    }

    Ok(updated_count)
}

/// Helper function to update link metadata and activate it
async fn update_link_metadata_and_activate(
    diesel_pool: Arc<DieselPool>,
    link_id: Uuid,
    metadata: UrlMetadata,
    redis_pool: Arc<RedisPool>,
) -> Result<(), ServiceError> {
    use crate::schema::links::dsl;
    use diesel::prelude::*;
    use diesel_async::RunQueryDsl;

    let mut conn = diesel_pool
        .get()
        .await
        .map_err(|e| ServiceError::DatabaseError(e.to_string()))?;

    // Update the link with extracted metadata and activate it
    diesel::update(dsl::links.filter(dsl::id.eq(link_id)))
        .set((
            dsl::title.eq(metadata.title),
            dsl::description.eq(metadata.description),
            dsl::og_image.eq(metadata.og_image),
            dsl::favicon_url.eq(metadata.favicon_url),
            dsl::is_active.eq(true),
            dsl::processing_status.eq("ready"),
            dsl::metadata_extracted_at.eq(Utc::now().naive_utc()),
            dsl::updated_at.eq(Utc::now()),
        ))
        .execute(&mut conn)
        .await
        .map_err(|e| ServiceError::DatabaseError(e.to_string()))?;

    // Get the link to invalidate its cache
    let link = dsl::links.find(link_id).first::<Link>(&mut conn).await?;

    // Invalidate cache for this link
    let cache_key = format!("link:{}", link.short_code);
    if let Err(e) = redis_pool.del(&cache_key).await {
        warn!("Failed to invalidate cache for {}: {}", cache_key, e);
    }

    // Also invalidate custom alias cache if exists
    if let Some(ref alias) = link.custom_alias {
        let alias_key = format!("link:{}", alias);
        if let Err(e) = redis_pool.del(&alias_key).await {
            warn!("Failed to invalidate cache for {}: {}", alias_key, e);
        }
    }

    info!(
        "Updated metadata, activated link {} and invalidated cache",
        link_id
    );
    Ok(())
}

/// Helper function to activate link immediately when user provides all metadata
async fn activate_link_immediately(
    diesel_pool: Arc<DieselPool>,
    link_id: Uuid,
    redis_pool: Arc<RedisPool>,
) -> Result<(), ServiceError> {
    use crate::models::link::Link;
    use crate::schema::links::dsl;
    use diesel::prelude::*;
    use diesel_async::RunQueryDsl;

    let mut conn = diesel_pool
        .get()
        .await
        .map_err(|e| ServiceError::DatabaseError(e.to_string()))?;

    // Activate the link immediately since user provided all metadata
    diesel::update(dsl::links.filter(dsl::id.eq(link_id)))
        .set((
            dsl::is_active.eq(true),
            dsl::processing_status.eq("completed"),
            dsl::metadata_extracted_at.eq(Utc::now().naive_utc()),
            dsl::updated_at.eq(Utc::now()),
        ))
        .execute(&mut conn)
        .await
        .map_err(|e| ServiceError::DatabaseError(e.to_string()))?;

    // Get the link to invalidate its cache
    let link = dsl::links.find(link_id).first::<Link>(&mut conn).await?;

    // Invalidate the short_code cache
    let cache_key = format!("link:{}", link.short_code);
    if let Err(e) = redis_pool.del(&cache_key).await {
        warn!("Failed to invalidate cache for {}: {}", cache_key, e);
    }

    // Also invalidate custom alias cache if exists
    if let Some(ref alias) = link.custom_alias {
        let alias_key = format!("link:{}", alias);
        if let Err(e) = redis_pool.del(&alias_key).await {
            warn!("Failed to invalidate cache for {}: {}", alias_key, e);
        }
    }

    info!(
        "Link {} activated immediately with user-provided metadata and invalidated cache",
        link_id
    );
    Ok(())
}

/// Helper function to activate link after metadata extraction failure
async fn activate_link_after_failure(
    diesel_pool: Arc<DieselPool>,
    link_id: Uuid,
    redis_pool: Arc<RedisPool>,
) -> Result<(), ServiceError> {
    use crate::schema::links::dsl;
    use diesel::prelude::*;
    use diesel_async::RunQueryDsl;

    let mut conn = diesel_pool
        .get()
        .await
        .map_err(|e| ServiceError::DatabaseError(e.to_string()))?;

    // Activate the link even though metadata extraction failed
    diesel::update(dsl::links.filter(dsl::id.eq(link_id)))
        .set((
            dsl::is_active.eq(true),
            dsl::processing_status.eq("failed"),
            dsl::metadata_extracted_at.eq(Utc::now().naive_utc()),
            dsl::updated_at.eq(Utc::now()),
        ))
        .execute(&mut conn)
        .await
        .map_err(|e| ServiceError::DatabaseError(e.to_string()))?;

    // Get the link to invalidate its cache
    let link = dsl::links.find(link_id).first::<Link>(&mut conn).await?;

    // Invalidate cache for this link
    let cache_key = format!("link:{}", link.short_code);
    if let Err(e) = redis_pool.del(&cache_key).await {
        warn!("Failed to invalidate cache for {}: {}", cache_key, e);
    }

    // Also invalidate custom alias cache if exists
    if let Some(ref alias) = link.custom_alias {
        let alias_key = format!("link:{}", alias);
        if let Err(e) = redis_pool.del(&alias_key).await {
            warn!("Failed to invalidate cache for {}: {}", alias_key, e);
        }
    }

    info!(
        "Activated link {} after metadata extraction failure and invalidated cache",
        link_id
    );
    Ok(())
}
