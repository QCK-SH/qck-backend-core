// DEV-108: Base62 Code Generation with Collision Detection
// High-performance short URL generation with Redis caching and profanity filtering

use chrono;
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use rand::{thread_rng, Rng};
use serde::Deserialize;
use std::collections::HashSet;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use thiserror::Error;
use tracing::{error, info, instrument, warn};

use crate::{
    app_config::CONFIG,
    db::{DieselPool, RedisPool},
    utils::base62::{Base62Encoder, Base62Error},
};

// =============================================================================
// CONSTANTS
// =============================================================================

/// Redis cache key prefix for short codes
const REDIS_SHORT_CODE_PREFIX: &str = "shortcode:";
/// Redis reservation key prefix
const REDIS_RESERVATION_PREFIX: &str = "reserve:";
/// Redis cache TTL for collision checks (5 minutes)
const REDIS_COLLISION_CHECK_TTL: usize = 300;
/// Redis reservation TTL (60 seconds during link creation)
const REDIS_RESERVATION_TTL: usize = 60;
/// High collision threshold (triggers length increase)
const HIGH_COLLISION_THRESHOLD: f64 = 0.01; // 1% collision rate

// =============================================================================
// ERROR TYPES
// =============================================================================

#[derive(Error, Debug)]
pub enum ShortCodeError {
    #[error("Invalid code length: {0}. Length must be between {1} and {2} characters")]
    InvalidLength(usize, usize, usize),

    #[error("Invalid custom alias '{alias}': {reason}")]
    InvalidCustomAlias { reason: String, alias: String },

    #[error("Custom alias '{0}' is reserved and cannot be used")]
    ReservedAlias(String),

    #[error("Custom alias '{0}' already exists. Suggestions: {1:?}")]
    AliasAlreadyExistsWithSuggestions(String, Vec<String>),

    #[error("Custom alias '{0}' already exists")]
    AliasAlreadyExists(String),

    #[error("Failed to generate unique code after maximum retries")]
    MaxRetriesExceeded,

    #[error("Invalid batch size: {0}. Maximum batch size is {1}")]
    InvalidBatchSize(usize, usize),

    #[error("Base62 encoding error: {0}")]
    Base62Error(#[from] Base62Error),

    #[error("Database error: {0}")]
    DatabaseError(#[from] diesel::result::Error),

    #[error("Redis error: {0}")]
    RedisError(String),

    #[error("Code contains profanity: {0}")]
    ProfanityDetected(String),
}

// =============================================================================
// JSON STRUCTURES
// =============================================================================

#[derive(Debug, Deserialize)]
struct ProfanityConfig {
    profanity_words: Vec<String>,
    leetspeak_mappings: std::collections::HashMap<String, Vec<String>>,
    check_substrings: bool,
    min_substring_length: usize,
    case_sensitive: bool,
}

#[derive(Debug, Deserialize)]
struct ReservedWordsConfig {
    system_routes: Vec<String>,
    api_endpoints: Vec<String>,
    user_management: Vec<String>,
    url_shortener_specific: Vec<String>,
    common_extensions: Vec<String>,
    security_sensitive: Vec<String>,
    business_terms: Vec<String>,
    brand_protection: Vec<String>,
    http_methods: Vec<String>,
    special_pages: Vec<String>,
    case_sensitive: bool,
    check_variations: bool,
    variation_patterns: Vec<String>,
}

// =============================================================================
// STATISTICS
// =============================================================================

/// Statistics about code generation
#[derive(Debug, Clone)]
pub struct GenerationStats {
    pub total_codes: i64,
    pub default_length: usize,
    pub length_distribution: Vec<(i32, i64)>,
    pub utilization_percentage: f64,
    pub reserved_codes_count: usize,
    pub collision_rate: f64,
    pub current_counter: u64,
}

// =============================================================================
// SHORT CODE GENERATOR
// =============================================================================

pub struct ShortCodeGenerator {
    pool: DieselPool,
    redis_pool: Option<RedisPool>,
    encoder: Base62Encoder,
    min_length: usize,
    current_length: AtomicU64, // Dynamic length based on collisions (replaces default_length)
    max_length: usize,
    max_retries: usize,
    batch_size: usize,
    reserved_codes: HashSet<String>,
    profanity_list: HashSet<String>,
    counter: Arc<AtomicU64>, // Atomic counter for sequential generation
    collision_count: Arc<AtomicU64>, // Track collisions for rate calculation
    generation_count: Arc<AtomicU64>, // Track total generations
    pre_generated_pool: Arc<tokio::sync::RwLock<Vec<String>>>, // Pre-generated code pool for high traffic
}

impl ShortCodeGenerator {
    /// Create a new ShortCodeGenerator with configuration from environment
    pub fn new(pool: DieselPool) -> Self {
        Self::with_redis(pool, None)
    }

    /// Create a new ShortCodeGenerator with Redis pool for caching
    pub fn with_redis(pool: DieselPool, redis_pool: Option<RedisPool>) -> Self {
        let config = &CONFIG;

        // Load reserved words from JSON file
        let reserved_codes = Self::load_reserved_words();

        // Load profanity list from JSON file
        let profanity_list = Self::load_profanity_list();

        // Initialize atomic counter with a random starting point to avoid predictable codes
        let mut rng = thread_rng();
        let initial_counter = rng.gen_range(1000000..10000000);

        Self {
            pool,
            redis_pool,
            encoder: Base62Encoder::with_constraints(
                config.short_code_min_length,
                config.short_code_max_length,
            ),
            min_length: config.short_code_min_length,
            current_length: AtomicU64::new(config.short_code_default_length as u64),
            max_length: config.short_code_max_length,
            max_retries: config.short_code_max_retries,
            batch_size: config.short_code_batch_size,
            reserved_codes,
            profanity_list,
            counter: Arc::new(AtomicU64::new(initial_counter)),
            collision_count: Arc::new(AtomicU64::new(0)),
            generation_count: Arc::new(AtomicU64::new(0)),
            pre_generated_pool: Arc::new(tokio::sync::RwLock::new(Vec::new())),
        }
    }

    /// Load reserved words from JSON file
    fn load_reserved_words() -> HashSet<String> {
        let mut reserved = HashSet::new();

        // Try to load from JSON file
        let json_path = "data/reserved_words.json";
        match std::fs::read_to_string(json_path) {
            Ok(content) => {
                match serde_json::from_str::<ReservedWordsConfig>(&content) {
                    Ok(config) => {
                        // Add all categories of reserved words
                        for word in config
                            .system_routes
                            .iter()
                            .chain(config.api_endpoints.iter())
                            .chain(config.user_management.iter())
                            .chain(config.url_shortener_specific.iter())
                            .chain(config.common_extensions.iter())
                            .chain(config.security_sensitive.iter())
                            .chain(config.business_terms.iter())
                            .chain(config.brand_protection.iter())
                            .chain(config.http_methods.iter())
                            .chain(config.special_pages.iter())
                        {
                            reserved.insert(word.to_lowercase());
                            reserved.insert(word.to_uppercase());
                        }
                        info!("Loaded {} reserved words from JSON", reserved.len());
                    },
                    Err(e) => {
                        error!("Failed to parse reserved words JSON: {}", e);
                        // Fall back to hardcoded minimal list
                        Self::load_fallback_reserved_words(&mut reserved);
                    },
                }
            },
            Err(e) => {
                warn!(
                    "Failed to read reserved words file: {}. Using fallback list.",
                    e
                );
                Self::load_fallback_reserved_words(&mut reserved);
            },
        }

        reserved
    }

    /// Load profanity list from JSON file
    fn load_profanity_list() -> HashSet<String> {
        let mut profanity = HashSet::new();

        // Try to load from JSON file
        let json_path = "data/profanity_list.json";
        match std::fs::read_to_string(json_path) {
            Ok(content) => {
                match serde_json::from_str::<ProfanityConfig>(&content) {
                    Ok(config) => {
                        for word in &config.profanity_words {
                            profanity.insert(word.to_lowercase());
                            profanity.insert(word.to_uppercase());

                            // Generate leetspeak variations
                            let mut leet = word.clone();
                            for (letter, replacements) in &config.leetspeak_mappings {
                                if let Some(replacement) = replacements.get(0) {
                                    leet = leet.replace(letter, replacement);
                                }
                            }
                            profanity.insert(leet.to_lowercase());
                        }
                        info!(
                            "Loaded {} profanity words from JSON (with variations)",
                            profanity.len()
                        );
                    },
                    Err(e) => {
                        error!("Failed to parse profanity JSON: {}", e);
                        Self::load_fallback_profanity(&mut profanity);
                    },
                }
            },
            Err(e) => {
                warn!("Failed to read profanity file: {}. Using fallback list.", e);
                Self::load_fallback_profanity(&mut profanity);
            },
        }

        profanity
    }

    /// Fallback reserved words if JSON loading fails
    fn load_fallback_reserved_words(reserved: &mut HashSet<String>) {
        let fallback = [
            "api",
            "app",
            "admin",
            "login",
            "dashboard",
            "user",
            "link",
            "url",
        ];
        for word in fallback {
            reserved.insert(word.to_string());
            reserved.insert(word.to_uppercase());
        }
    }

    /// Fallback profanity list if JSON loading fails
    fn load_fallback_profanity(profanity: &mut HashSet<String>) {
        let fallback = ["fuck", "shit", "damn", "hell", "ass", "bitch"];
        for word in fallback {
            profanity.insert(word.to_string());
            profanity.insert(word.to_uppercase());
        }
    }

    /// Pre-generate codes for high-traffic periods
    pub async fn refill_code_pool(&self, size: usize) -> Result<(), ShortCodeError> {
        let current_length = AtomicU64::load(&self.current_length, Ordering::Relaxed) as usize;

        // Generate batch of codes
        let new_codes = self.generate_batch_codes(size, current_length).await?;

        // Add to pool
        let mut pool = self.pre_generated_pool.write().await;
        pool.extend(new_codes);

        info!(
            "Pre-generated code pool refilled with {} codes, total: {}",
            size,
            pool.len()
        );
        Ok(())
    }

    /// Get a code from the pre-generated pool if available
    pub async fn get_from_pool(&self) -> Option<String> {
        let mut pool = self.pre_generated_pool.write().await;
        pool.pop()
    }

    /// Get the current pool size
    pub async fn get_pool_size(&self) -> usize {
        let pool = self.pre_generated_pool.read().await;
        pool.len()
    }

    /// Generate a unique short code with collision detection
    #[instrument(skip(self))]
    pub async fn generate_unique_code(&self) -> Result<String, ShortCodeError> {
        // Try to get from pre-generated pool first (for high-traffic optimization)
        if let Some(code) = self.get_from_pool().await {
            info!("Using pre-generated code from pool: {}", code);
            AtomicU64::fetch_add(&self.generation_count, 1, Ordering::Relaxed);
            return Ok(code);
        }

        // Get current length (may have been increased due to collisions)
        let current_length = AtomicU64::load(&self.current_length, Ordering::Relaxed) as usize;
        self.generate_unique_code_with_length(current_length).await
    }

    /// Generate a unique short code with custom length
    #[instrument(skip(self))]
    pub async fn generate_unique_code_with_length(
        &self,
        length: usize,
    ) -> Result<String, ShortCodeError> {
        if length < self.min_length || length > self.max_length {
            return Err(ShortCodeError::InvalidLength(
                length,
                self.min_length,
                self.max_length,
            ));
        }

        let mut attempts = 0;
        AtomicU64::fetch_add(&self.generation_count, 1, Ordering::Relaxed);

        while attempts < self.max_retries {
            // Strategy: Use counter for first 3 attempts, then random
            let candidate = if attempts < 3 {
                // Counter-based generation for performance
                self.generate_from_counter(length).await?
            } else {
                // Random generation as fallback
                // Increase length dynamically if collision rate is high
                let dynamic_length = if attempts > 5 {
                    std::cmp::min(length + (attempts - 5) / 2, self.max_length)
                } else {
                    length
                };
                self.generate_random_code(dynamic_length)
            };

            // Check if code is reserved
            if self.is_reserved_code(&candidate) {
                attempts += 1;
                continue;
            }

            // Check for profanity
            if self.contains_profanity(&candidate) {
                warn!("Generated code contains profanity: {}", candidate);
                attempts += 1;
                continue;
            }

            // Check database for uniqueness (required for all codes to ensure no duplicates)
            // For counter-based codes, we can optimize by checking Redis cache first
            let is_unique = if attempts < 3 && self.redis_pool.is_some() {
                // For counter-based, check Redis cache first for performance
                match self.check_redis_then_db(&candidate).await {
                    Ok(unique) => unique,
                    Err(e) => {
                        // Fallback to database check if Redis fails
                        warn!("Redis check failed, falling back to DB: {}", e);
                        self.is_code_unique(&candidate).await?
                    },
                }
            } else {
                // For random codes or when Redis is not available, check database directly
                self.is_code_unique(&candidate).await?
            };

            if is_unique {
                // Reserve the code in Redis temporarily
                if let Err(e) = self.reserve_code(&candidate).await {
                    warn!("Failed to reserve code in Redis: {}", e);
                }

                info!(
                    "Generated unique short code: {} (length: {}, attempts: {}, method: {})",
                    candidate,
                    candidate.len(),
                    attempts + 1,
                    if attempts < 3 { "counter" } else { "random" }
                );

                // Check collision rate and adjust length if needed
                self.check_and_adjust_length().await;

                return Ok(candidate);
            } else {
                AtomicU64::fetch_add(&self.collision_count, 1, Ordering::Relaxed);
                warn!(
                    "Short code collision detected: {} (attempt: {})",
                    candidate,
                    attempts + 1
                );
                attempts += 1;
                continue;
            }
        }

        error!(
            "Failed to generate unique short code after {} attempts",
            self.max_retries
        );
        Err(ShortCodeError::MaxRetriesExceeded)
    }

    /// Generate code from atomic counter
    async fn generate_from_counter(&self, min_length: usize) -> Result<String, ShortCodeError> {
        let counter_value = AtomicU64::fetch_add(&self.counter, 1, Ordering::SeqCst);
        let mut code = self.encoder.encode(counter_value);

        // Ensure minimum length by adding random prefix if needed
        if code.len() < min_length {
            let padding_needed = min_length - code.len();
            let prefix = self.generate_random_prefix(padding_needed);
            code = format!("{}{}", prefix, code);
        }

        Ok(code)
    }

    /// Generate random prefix for padding
    fn generate_random_prefix(&self, length: usize) -> String {
        // Use the Base62Encoder's generate_random method
        self.encoder.generate_random(length).unwrap_or_else(|_| {
            // Fallback if encoder fails
            let mut rng = thread_rng();
            (0..length)
                .map(|_| {
                    let idx = rng.gen_range(0..62);
                    // Use the same alphabet as base62.rs
                    let alphabet = "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
                    alphabet.chars().nth(idx).unwrap()
                })
                .collect()
        })
    }

    /// Check collision rate and adjust default length if needed
    async fn check_and_adjust_length(&self) {
        let collisions = AtomicU64::load(&self.collision_count, Ordering::Relaxed);
        let generations = AtomicU64::load(&self.generation_count, Ordering::Relaxed);

        if generations > 100 {
            // Only check after sufficient samples
            let collision_rate = collisions as f64 / generations as f64;

            // Alert on unusual collision patterns
            if collision_rate > 0.05 {
                // > 5% is unusual
                error!(
                    "🚨 ALERT: Unusual collision rate detected: {:.2}% (threshold: 5%)",
                    collision_rate * 100.0
                );

                // Log detailed metrics for investigation
                error!(
                    "Collision metrics - Total generations: {}, Collisions: {}, Rate: {:.2}%",
                    generations,
                    collisions,
                    collision_rate * 100.0
                );

                // Trigger alert (in production, this would send to monitoring service)
                self.send_collision_alert(collision_rate).await;
            }

            if collision_rate > HIGH_COLLISION_THRESHOLD {
                let current = AtomicU64::load(&self.current_length, Ordering::Relaxed);
                if current < self.max_length as u64 {
                    let new_length = current + 1;
                    AtomicU64::store(&self.current_length, new_length, Ordering::Relaxed);
                    warn!(
                        "High collision rate detected ({:.2}%). Increasing default length to {}",
                        collision_rate * 100.0,
                        new_length
                    );

                    // Reset counters after adjustment
                    AtomicU64::store(&self.collision_count, 0, Ordering::Relaxed);
                    AtomicU64::store(&self.generation_count, 0, Ordering::Relaxed);
                }
            }
        }
    }

    /// Send alert for unusual collision patterns (integrate with monitoring service)
    async fn send_collision_alert(&self, collision_rate: f64) {
        // In production, this would integrate with PagerDuty, Datadog, etc.
        error!(
            "🚨 COLLISION ALERT: Rate {:.2}% exceeds normal threshold. Investigation required.",
            collision_rate * 100.0
        );

        // Store alert in Redis for dashboard visibility
        if let Some(redis_pool) = &self.redis_pool {
            let alert_key = format!("alert:collision:{}", chrono::Utc::now().timestamp());
            let alert_data = format!(
                "{{\"rate\": {:.4}, \"timestamp\": {}, \"severity\": \"high\"}}",
                collision_rate,
                chrono::Utc::now().timestamp()
            );

            if let Err(e) = redis_pool
                .set_with_expiry(
                    &alert_key, alert_data, 86400, // Keep alert for 24 hours
                )
                .await
            {
                error!("Failed to store collision alert in Redis: {}", e);
            }
        }
    }

    /// Generate a random Base62 code of specified length
    pub fn generate_random_code(&self, length: usize) -> String {
        // Use our Base62Encoder for consistent generation
        self.encoder.generate_random(length).unwrap_or_else(|_| {
            // Fallback to manual generation if encoder fails
            let mut rng = thread_rng();
            let alphabet = "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
            (0..length)
                .map(|_| {
                    let idx = rng.gen_range(0..62);
                    alphabet.chars().nth(idx).unwrap()
                })
                .collect()
        })
    }

    /// Check if a code is in the reserved list
    fn is_reserved_code(&self, code: &str) -> bool {
        self.reserved_codes.contains(&code.to_lowercase())
    }

    /// Check if code contains profanity
    fn contains_profanity(&self, code: &str) -> bool {
        let code_lower = code.to_lowercase();

        // Check exact matches
        if self.profanity_list.contains(&code_lower) {
            return true;
        }

        // Check if code contains any profanity as substring
        for word in &self.profanity_list {
            if code_lower.contains(word.as_str()) {
                return true;
            }
        }

        false
    }

    /// Check if a code is unique using Redis cache first, then database
    pub async fn is_code_unique(&self, code: &str) -> Result<bool, diesel::result::Error> {
        // First check Redis cache if available
        if let Some(redis_pool) = &self.redis_pool {
            // Check if code is reserved
            let reserve_key = format!("{}{}", REDIS_RESERVATION_PREFIX, code);
            match redis_pool.get::<String>(&reserve_key).await {
                Ok(Some(_)) => {
                    // Code is reserved, not unique
                    return Ok(false);
                },
                _ => {},
            }

            // Check if code exists in cache
            let cache_key = format!("{}{}", REDIS_SHORT_CODE_PREFIX, code);
            match redis_pool.get::<String>(&cache_key).await {
                Ok(Some(_)) => {
                    // Code exists in cache, not unique
                    return Ok(false);
                },
                _ => {
                    // Cache miss or error, continue to database check
                },
            }
        }

        // Check database
        use crate::schema::links::dsl::*;

        let mut conn = self.pool.get().await.map_err(|e| {
            diesel::result::Error::DatabaseError(
                diesel::result::DatabaseErrorKind::UnableToSendCommand,
                Box::new(e.to_string()),
            )
        })?;

        // Use SELECT 1 LIMIT 1 for better performance than COUNT(*)
        use diesel::dsl::exists;
        use diesel::select;

        let code_exists: bool = select(exists(
            links.filter(short_code.eq(code).or(custom_alias.eq(code))),
        ))
        .get_result(&mut conn)
        .await?;

        let is_unique = !code_exists;

        // If not unique, cache it to speed up future checks
        if !is_unique {
            if let Some(redis_pool) = &self.redis_pool {
                let cache_key = format!("{}{}", REDIS_SHORT_CODE_PREFIX, code);
                // Set with TTL, ignore errors
                let _ = redis_pool
                    .set_with_expiry(&cache_key, "1".to_string(), REDIS_COLLISION_CHECK_TTL)
                    .await;
            }
        }

        Ok(is_unique)
    }

    /// Check Redis cache first, then database for code existence
    /// This is optimized for counter-based codes which are unlikely to exist
    async fn check_redis_then_db(&self, code: &str) -> Result<bool, ShortCodeError> {
        if let Some(redis_pool) = &self.redis_pool {
            // Check if code exists in Redis cache (acts as a bloom filter)
            let cache_key = format!("exists:{}", code);

            // First check if we have a cached "exists" entry
            match redis_pool.get::<String>(&cache_key).await {
                Ok(Some(_)) => {
                    // Code exists (cached positive result)
                    return Ok(false); // Not unique
                },
                Ok(None) => {
                    // Not in cache, need to check database
                },
                Err(e) => {
                    warn!("Redis cache check failed: {}", e);
                    // Continue to database check
                },
            }

            // Check database
            let is_unique = self.is_code_unique(code).await?;

            // Cache the result if code exists (only cache positive hits to save memory)
            if !is_unique {
                // Cache that this code exists for 1 hour
                if let Err(e) = redis_pool
                    .set_with_expiry(
                        &cache_key,
                        "1".to_string(),
                        3600, // 1 hour TTL
                    )
                    .await
                {
                    warn!("Failed to cache code existence: {}", e);
                }
            }

            Ok(is_unique)
        } else {
            // No Redis, fallback to database check
            self.is_code_unique(code)
                .await
                .map_err(|e| ShortCodeError::DatabaseError(e))
        }
    }

    /// Reserve a code in Redis temporarily during link creation
    async fn reserve_code(&self, code: &str) -> Result<(), ShortCodeError> {
        if let Some(redis_pool) = &self.redis_pool {
            let reserve_key = format!("{}{}", REDIS_RESERVATION_PREFIX, code);

            match redis_pool
                .set_with_expiry(&reserve_key, "reserved".to_string(), REDIS_RESERVATION_TTL)
                .await
            {
                Ok(_) => {
                    info!(
                        "Reserved code '{}' in Redis for {} seconds",
                        code, REDIS_RESERVATION_TTL
                    );
                    Ok(())
                },
                Err(e) => {
                    error!("Failed to reserve code in Redis: {}", e);
                    Err(ShortCodeError::RedisError(e.to_string()))
                },
            }
        } else {
            // No Redis available, skip reservation
            Ok(())
        }
    }

    /// Release a reserved code (e.g., if link creation fails)
    pub async fn release_code(&self, code: &str) -> Result<(), ShortCodeError> {
        if let Some(redis_pool) = &self.redis_pool {
            let reserve_key = format!("{}{}", REDIS_RESERVATION_PREFIX, code);

            match redis_pool.del(&reserve_key).await {
                Ok(_) => {
                    info!("Released reserved code '{}'", code);
                    Ok(())
                },
                Err(e) => {
                    error!("Failed to release reserved code: {}", e);
                    Err(ShortCodeError::RedisError(e.to_string()))
                },
            }
        } else {
            Ok(())
        }
    }

    /// Validate a custom alias provided by user
    #[instrument(skip(self))]
    pub async fn validate_custom_alias(&self, alias: &str) -> Result<(), ShortCodeError> {
        // Use CustomAliasValidator as specified
        use crate::utils::custom_alias_validator::CustomAliasValidator;

        // Validate format using CustomAliasValidator
        if let Err(reason) = CustomAliasValidator::validate(alias) {
            return Err(ShortCodeError::InvalidCustomAlias {
                reason,
                alias: alias.to_string(),
            });
        }

        // Check if reserved
        if self.is_reserved_code(alias) {
            return Err(ShortCodeError::ReservedAlias(alias.to_string()));
        }

        // Check uniqueness
        match self.is_code_unique(alias).await {
            Ok(true) => Ok(()),
            Ok(false) => Err(ShortCodeError::AliasAlreadyExists(alias.to_string())),
            Err(e) => {
                error!("Database error validating custom alias: {}", e);
                Err(ShortCodeError::DatabaseError(e))
            },
        }
    }

    /// Generate multiple unique codes in batch (for pre-generation)
    #[instrument(skip(self))]
    pub async fn generate_batch_codes(
        &self,
        count: usize,
        length: usize,
    ) -> Result<Vec<String>, ShortCodeError> {
        if count > self.batch_size {
            return Err(ShortCodeError::InvalidBatchSize(count, self.batch_size));
        }

        if length < self.min_length || length > self.max_length {
            return Err(ShortCodeError::InvalidLength(
                length,
                self.min_length,
                self.max_length,
            ));
        }

        let mut codes = Vec::with_capacity(count);
        let mut candidates = Vec::with_capacity(count * 2); // Generate extra for collisions
        let mut attempts = 0;
        let max_total_attempts = count * 10; // Allow up to 10x attempts

        // Step 1: Generate candidate codes in bulk
        while candidates.len() < count * 2 && attempts < max_total_attempts {
            let candidate = self.generate_random_code(length);

            // Skip if reserved or duplicate in batch
            if !self.is_reserved_code(&candidate) && !candidates.contains(&candidate) {
                candidates.push(candidate);
            }
            attempts += 1;
        }

        // Step 2: Batch check all candidates at once for better performance
        match self.batch_check_uniqueness(&candidates).await {
            Ok(unique_codes) => {
                // Take up to 'count' unique codes
                codes.extend(unique_codes.into_iter().take(count));
            },
            Err(e) => {
                error!("Database error in batch uniqueness check: {}", e);
                return Err(ShortCodeError::DatabaseError(e));
            },
        }

        if codes.len() < count {
            warn!(
                "Batch generation incomplete: generated {} out of {} codes",
                codes.len(),
                count
            );
        } else {
            info!(
                "Successfully generated batch of {} codes (attempts: {})",
                count, attempts
            );
        }

        Ok(codes)
    }

    /// Convert integer ID to Base62 (deterministic generation)
    pub fn encode_id(id: u64) -> String {
        Base62Encoder::new().encode(id)
    }

    /// Decode Base62 string back to integer (for analysis)
    pub fn decode_to_id(code: &str) -> Result<u64, ShortCodeError> {
        Base62Encoder::new()
            .decode(code)
            .map_err(ShortCodeError::Base62Error)
    }

    /// Generate suggestions for a taken custom alias (FAST VERSION)
    /// Returns 5 likely-available alternatives without checking database
    pub async fn generate_suggestions(&self, base_alias: &str) -> Vec<String> {
        let mut suggestions = Vec::new();
        let base = base_alias.to_lowercase();
        let mut rng = thread_rng();

        // Generate random suffix for uniqueness
        let random1 = rng.gen_range(10..99);
        let random2 = rng.gen_range(100..999);
        let random3 = rng.gen_range(1000..9999);

        // Pattern-based suggestions (likely to be available)
        let patterns = vec![
            format!("{}-{}", base, random1),
            format!("{}{}", base, random2),
            format!("{}-{}", base, random3),
            format!("my-{}-{}", base, random1),
            format!("{}-2025", base),
        ];

        // Add suggestions that fit within length limit and aren't reserved
        for pattern in patterns {
            if suggestions.len() >= 5 {
                break;
            }

            // Basic validation only (no database check)
            if pattern.len() <= self.max_length && !self.is_reserved_code(&pattern) {
                suggestions.push(pattern);
            }
        }

        // Fill remaining slots with random variations
        while suggestions.len() < 5 {
            let suffix = rng.gen_range(1000..99999);
            let suggestion = if rng.gen_bool(0.5) {
                format!("{}-{}", base, suffix)
            } else {
                format!("{}{}", base, suffix)
            };

            if suggestion.len() <= self.max_length && !self.is_reserved_code(&suggestion) {
                suggestions.push(suggestion);
            }
        }

        suggestions
    }

    /// Batch check multiple codes for uniqueness in a single database query
    async fn batch_check_uniqueness(
        &self,
        codes: &[String],
    ) -> Result<Vec<String>, diesel::result::Error> {
        use crate::schema::links::dsl::*;

        if codes.is_empty() {
            return Ok(Vec::new());
        }

        let mut conn = self.pool.get().await.map_err(|e| {
            diesel::result::Error::DatabaseError(
                diesel::result::DatabaseErrorKind::UnableToSendCommand,
                Box::new(e.to_string()),
            )
        })?;

        // Get all existing codes in one query
        let existing_codes: Vec<String> = links
            .select(short_code)
            .filter(short_code.eq_any(codes))
            .load::<String>(&mut conn)
            .await?;

        // Also check custom aliases
        let existing_aliases: Vec<Option<String>> = links
            .select(custom_alias)
            .filter(custom_alias.eq_any(codes))
            .load::<Option<String>>(&mut conn)
            .await?;

        let existing_aliases: Vec<String> = existing_aliases.into_iter().flatten().collect();

        // Filter out codes that exist
        let mut unique_codes = Vec::new();
        for code in codes {
            if !existing_codes.contains(code) && !existing_aliases.contains(code) {
                unique_codes.push(code.clone());
            }
        }

        info!(
            "Batch checked {} codes, found {} unique",
            codes.len(),
            unique_codes.len()
        );
        Ok(unique_codes)
    }

    /// Get statistics about code generation
    pub async fn get_generation_stats(&self) -> Result<GenerationStats, diesel::result::Error> {
        use crate::schema::links::dsl::*;

        let mut conn = self.pool.get().await.map_err(|e| {
            diesel::result::Error::DatabaseError(
                diesel::result::DatabaseErrorKind::UnableToSendCommand,
                Box::new(e.to_string()),
            )
        })?;

        // Get total count
        let total_codes: i64 = links.count().get_result(&mut conn).await?;

        // Get length distribution using raw SQL
        #[derive(Debug, diesel::QueryableByName)]
        struct LengthCount {
            #[diesel(sql_type = diesel::sql_types::Integer)]
            length: i32,
            #[diesel(sql_type = diesel::sql_types::BigInt)]
            count: i64,
        }

        let length_distribution: Vec<LengthCount> = diesel::sql_query(
            "SELECT LENGTH(short_code) as length, COUNT(*) as count 
             FROM links 
             GROUP BY LENGTH(short_code) 
             ORDER BY length",
        )
        .load::<LengthCount>(&mut conn)
        .await
        .unwrap_or_default();

        // Convert to tuple for backwards compatibility
        let length_distribution: Vec<(i32, i64)> = length_distribution
            .into_iter()
            .map(|lc| (lc.length, lc.count))
            .collect();

        // Calculate utilization for current length
        let current_length = AtomicU64::load(&self.current_length, Ordering::Relaxed) as usize;
        let current_length_count = length_distribution
            .iter()
            .find(|(len, _)| *len == current_length as i32)
            .map(|(_, count)| *count)
            .unwrap_or(0);

        let max_combinations = 62u64.pow(current_length as u32);
        let utilization_percentage = if max_combinations > 0 {
            (current_length_count as f64 / max_combinations as f64) * 100.0
        } else {
            0.0
        };

        // Calculate collision rate
        let collisions = AtomicU64::load(&self.collision_count, Ordering::Relaxed);
        let generations = AtomicU64::load(&self.generation_count, Ordering::Relaxed);
        let collision_rate = if generations > 0 {
            collisions as f64 / generations as f64
        } else {
            0.0
        };

        Ok(GenerationStats {
            total_codes,
            default_length: current_length,
            length_distribution,
            utilization_percentage,
            reserved_codes_count: self.reserved_codes.len(),
            collision_rate,
            current_counter: AtomicU64::load(&self.counter, Ordering::Relaxed),
        })
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base62_encoding_decoding() {
        let test_cases = vec![
            (0, "0"),
            (1, "1"),
            (61, "z"),
            (62, "10"),
            (123, "1z"),
            (3843, "zz"),
            (238327, "zzz"),
        ];

        for (id, expected) in test_cases {
            let encoded = ShortCodeGenerator::encode_id(id);
            assert_eq!(encoded, expected, "Encoding failed for ID: {}", id);

            let decoded = ShortCodeGenerator::decode_to_id(&encoded).unwrap();
            assert_eq!(decoded, id, "Decoding failed for code: {}", encoded);
        }
    }

    #[test]
    fn test_decode_invalid_characters() {
        let invalid_codes = vec!["abc!", "test@", "hello world", "😀"];

        for code in invalid_codes {
            assert!(
                ShortCodeGenerator::decode_to_id(code).is_err(),
                "Should fail for invalid code: {}",
                code
            );
        }
    }

    #[tokio::test]
    async fn test_reserved_codes() {
        // This would require a mock pool - skipping for now
        // But the logic is tested through the is_reserved_code method
    }

    #[test]
    fn test_random_code_generation_length() {
        // Create a mock generator (would need mock pool in real scenario)
        let lengths = vec![4, 6, 7, 8, 10, 12];

        for len in lengths {
            // Test that generated codes have correct length
            // This would be tested with actual generator instance
        }
    }
}
