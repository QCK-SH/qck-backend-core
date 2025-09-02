// DEV-68: Link Management API - Core link model
// DEV-124: Base62 short code support
// DEV-105: Link creation with metadata

use chrono::{DateTime, Utc};
use diesel::prelude::*;
use lazy_static::lazy_static;
use regex::Regex;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;
use validator::Validate;

use crate::schema::links;
use crate::services::link::LinkClickStats;

// =============================================================================
// DATABASE MODELS
// =============================================================================

/// Link model representing a database record
#[derive(Debug, Clone, Queryable, Selectable, Identifiable, Serialize, Deserialize, ToSchema)]
#[diesel(table_name = links)]
#[diesel(check_for_backend(diesel::pg::Pg))]
#[schema(example = json!({
    "id": "123e4567-e89b-12d3-a456-426614174000",
    "short_code": "abc123",
    "original_url": "https://example.com/very/long/url",
    "user_id": "123e4567-e89b-12d3-a456-426614174000",
    "expires_at": null,
    "click_count": 42,
    "is_active": true,
    "created_at": "2024-01-01T12:00:00Z",
    "updated_at": "2024-01-01T12:00:00Z",
    "custom_alias": "my-link",
    "last_accessed_at": "2024-01-01T12:00:00Z",
    "metadata": {
        "title": "Example Site",
        "description": "A great example website",
        "domain": "example.com",
        "is_safe": true,
        "tags": ["example", "test"]
    }
}))]
pub struct Link {
    pub id: Uuid,
    pub user_id: Uuid,
    pub short_code: String,
    pub original_url: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub tags: Option<Vec<Option<String>>>,
    pub custom_alias: Option<String>,
    pub is_active: bool,
    pub expires_at: Option<DateTime<Utc>>,
    pub password_hash: Option<String>,
    pub last_accessed_at: Option<DateTime<Utc>>,
    pub og_image: Option<String>,
    pub favicon_url: Option<String>,
    pub processing_status: String,
    pub metadata_extracted_at: Option<DateTime<Utc>>,
    pub utm_source: Option<String>,
    pub utm_medium: Option<String>,
    pub utm_campaign: Option<String>,
    pub utm_term: Option<String>,
    pub utm_content: Option<String>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// New link for insertion
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = links)]
pub struct NewLink {
    pub id: Uuid,
    pub user_id: Uuid,
    pub short_code: String,
    pub original_url: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub tags: Option<Vec<Option<String>>>,
    pub custom_alias: Option<String>,
    pub is_active: bool,
    pub expires_at: Option<DateTime<Utc>>,
    pub password_hash: Option<String>,
    pub last_accessed_at: Option<DateTime<Utc>>,
    pub og_image: Option<String>,
    pub favicon_url: Option<String>,
    pub processing_status: String,
    pub metadata_extracted_at: Option<DateTime<Utc>>,
    pub utm_source: Option<String>,
    pub utm_medium: Option<String>,
    pub utm_campaign: Option<String>,
    pub utm_term: Option<String>,
    pub utm_content: Option<String>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Update link fields
#[derive(Debug, Clone, AsChangeset)]
#[diesel(table_name = links)]
pub struct UpdateLink {
    pub original_url: Option<String>,
    pub title: Option<Option<String>>,
    pub description: Option<Option<String>>,
    pub tags: Option<Option<Vec<Option<String>>>>,
    pub expires_at: Option<Option<DateTime<Utc>>>,
    pub is_active: Option<bool>,
    pub password_hash: Option<Option<String>>,
    pub utm_source: Option<Option<String>>,
    pub utm_medium: Option<Option<String>>,
    pub utm_campaign: Option<Option<String>>,
    pub utm_term: Option<Option<String>>,
    pub utm_content: Option<Option<String>>,
    pub updated_at: DateTime<Utc>,
    pub processing_status: Option<String>,
    pub metadata_extracted_at: Option<Option<chrono::NaiveDateTime>>,
    pub og_image: Option<Option<String>>,
    pub favicon_url: Option<Option<String>>,
}

// =============================================================================
// REQUEST/RESPONSE DTOs
// =============================================================================

/// Request to create a new link
#[derive(Debug, Clone, Deserialize, Validate, ToSchema)]
#[schema(example = json!({
    "url": "https://example.com/very/long/url/that/needs/shortening",
    "custom_alias": "my-custom-link",
    "title": "My Awesome Link",
    "description": "This is a description of my link",
    "og_image": "https://example.com/og-image.jpg",
    "favicon_url": "https://example.com/favicon.ico",
    "expires_at": "2024-12-31T23:59:59Z",
    "tags": ["work", "important"],
    "is_password_protected": false,
    "password": null
}))]
pub struct CreateLinkRequest {
    #[validate(url(message = "Invalid URL format"))]
    #[validate(length(max = 8192, message = "URL must be less than 8192 characters"))]
    pub url: String,

    #[validate(length(min = 3, max = 50, message = "Custom alias must be 3-50 characters"))]
    #[validate(regex(
        path = "CUSTOM_ALIAS_REGEX",
        message = "Custom alias can only contain letters, numbers, hyphens, and underscores"
    ))]
    pub custom_alias: Option<String>,

    #[validate(length(max = 200, message = "Title must be less than 200 characters"))]
    pub title: Option<String>,

    #[validate(length(max = 500, message = "Description must be less than 500 characters"))]
    pub description: Option<String>,

    #[validate(url(message = "Invalid OG image URL format"))]
    #[validate(length(max = 2048, message = "OG image URL must be less than 2048 characters"))]
    pub og_image: Option<String>,

    #[validate(url(message = "Invalid favicon URL format"))]
    #[validate(length(max = 2048, message = "Favicon URL must be less than 2048 characters"))]
    pub favicon_url: Option<String>,

    pub expires_at: Option<DateTime<Utc>>,

    #[serde(default)]
    pub tags: Vec<String>,

    #[serde(default)]
    pub is_password_protected: bool,

    pub password: Option<String>,
}

lazy_static! {
    static ref CUSTOM_ALIAS_REGEX: Regex = Regex::new(r"^[a-zA-Z0-9][a-zA-Z0-9_-]*$").unwrap();
}

/// Custom validation for CreateLinkRequest
impl CreateLinkRequest {
    pub fn validate_custom(&self) -> Result<(), String> {
        // Validate password if protection is enabled
        if self.is_password_protected && self.password.is_none() {
            return Err("Password is required when password protection is enabled".to_string());
        }

        // Validate expiration date is in the future
        if let Some(expires_at) = self.expires_at {
            if expires_at <= Utc::now() {
                return Err("Expiration date must be in the future".to_string());
            }
        }

        // Validate tags
        if self.tags.len() > 10 {
            return Err("Maximum 10 tags allowed".to_string());
        }

        for tag in &self.tags {
            if tag.len() > 30 {
                return Err("Each tag must be less than 30 characters".to_string());
            }
        }

        Ok(())
    }

    /// Trim and sanitize input fields
    pub fn sanitize(&mut self) {
        self.url = self.url.trim().to_string();
        self.custom_alias = self.custom_alias.as_ref().map(|s| s.trim().to_string());
        self.title = self.title.as_ref().map(|s| s.trim().to_string());
        self.description = self.description.as_ref().map(|s| s.trim().to_string());
        self.og_image = self.og_image.as_ref().map(|s| s.trim().to_string());
        self.favicon_url = self.favicon_url.as_ref().map(|s| s.trim().to_string());
        self.tags = self.tags.iter().map(|s| s.trim().to_string()).collect();
    }
}

/// Request to update an existing link
#[derive(Debug, Clone, Deserialize, Validate, ToSchema)]
#[schema(example = json!({
    "url": "https://updated-example.com/new/url",
    "title": "Updated Title",
    "description": "Updated description",
    "og_image": "https://updated-example.com/new-og-image.jpg",
    "favicon_url": "https://updated-example.com/new-favicon.ico",
    "expires_at": "2024-12-31T23:59:59Z",
    "is_active": true,
    "tags": ["updated", "modified"],
    "is_password_protected": false,
    "password": null
}))]
pub struct UpdateLinkRequest {
    #[validate(url(message = "Invalid URL format"))]
    #[validate(length(max = 8192, message = "URL must be less than 8192 characters"))]
    pub url: Option<String>,

    #[validate(length(max = 200, message = "Title must be less than 200 characters"))]
    pub title: Option<String>,

    #[validate(length(max = 500, message = "Description must be less than 500 characters"))]
    pub description: Option<String>,

    #[validate(url(message = "Invalid OG image URL format"))]
    #[validate(length(max = 2048, message = "OG image URL must be less than 2048 characters"))]
    pub og_image: Option<String>,

    #[validate(url(message = "Invalid favicon URL format"))]
    #[validate(length(max = 2048, message = "Favicon URL must be less than 2048 characters"))]
    pub favicon_url: Option<String>,

    pub expires_at: Option<Option<DateTime<Utc>>>,

    pub is_active: Option<bool>,

    pub tags: Option<Vec<String>>,

    pub is_password_protected: Option<bool>,

    pub password: Option<String>,
}

/// Link response for API
#[derive(Debug, Clone, Serialize, ToSchema)]
#[schema(example = json!({
    "id": "123e4567-e89b-12d3-a456-426614174000",
    "short_code": "abc123",
    "original_url": "https://example.com/very/long/url",
    "short_url": "https://qck.sh/r/abc123",
    "title": "Example Site",
    "description": "A great example website",
    "click_count": 42,
    "expires_at": null,
    "created_at": "2024-01-01T12:00:00Z",
    "updated_at": "2024-01-01T12:00:00Z",
    "is_active": true,
    "qr_code_url": "https://qck.sh/api/v1/qr/abc123",
    "tags": ["example", "test"],
    "is_password_protected": false,
    "metadata": {
        "title": "Example Site",
        "description": "A great example website",
        "domain": "example.com",
        "is_safe": true,
        "tags": ["example", "test"]
    }
}))]
pub struct LinkResponse {
    pub id: Uuid,
    pub short_code: String,
    pub original_url: String,
    pub short_url: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub is_active: bool,
    pub tags: Vec<String>,
    pub is_password_protected: bool,
    pub metadata: LinkMetadata,
    // Stats from ClickHouse
    pub total_clicks: u64,
    pub unique_visitors: u64,
    pub bot_clicks: u64,
    pub last_accessed_at: Option<DateTime<Utc>>,
}

/// Parameters for listing links
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct ListLinksParams {
    pub page: u32,
    pub per_page: u32,
    pub sort_by: Option<String>,
    pub filter: LinkFilter,
}

impl ListLinksParams {
    pub fn offset(&self) -> i64 {
        ((self.page - 1) * self.per_page) as i64
    }

    pub fn limit(&self) -> i64 {
        self.per_page as i64
    }
}

/// Paginated response for links
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct PaginatedLinks {
    pub links: Vec<Link>,
    pub total: u64,
    pub page: u32,
    pub per_page: u32,
}

/// Link metadata extracted from URL
#[derive(Debug, Clone, Serialize, Deserialize, Default, ToSchema)]
#[schema(example = json!({
    "title": "Example Site",
    "description": "A great example website",
    "favicon_url": "https://example.com/favicon.ico",
    "og_image": "https://example.com/og-image.jpg",
    "domain": "example.com",
    "is_safe": true,
    "tags": ["example", "test"],
    "password_hash": null
}))]
pub struct LinkMetadata {
    pub title: Option<String>,
    pub description: Option<String>,
    pub favicon_url: Option<String>,
    pub og_image: Option<String>,
    pub domain: String,
    pub is_safe: bool,
    pub tags: Vec<String>,
    pub password_hash: Option<String>,
}

impl LinkMetadata {
    /// Create metadata from request and extracted info
    pub fn from_request(request: &CreateLinkRequest, extracted: Option<ExtractedMetadata>) -> Self {
        let domain = extract_domain(&request.url).unwrap_or_default();

        let (title, description, favicon_url, og_image) = if let Some(extracted) = extracted {
            (
                request.title.clone().or(extracted.title),
                request.description.clone().or(extracted.description),
                extracted.favicon_url,
                extracted.og_image,
            )
        } else {
            (
                request.title.clone(),
                request.description.clone(),
                None,
                None,
            )
        };

        let password_hash = if request.is_password_protected {
            request.password.as_ref().map(|p| {
                // Hash password with bcrypt (this should be done in service layer)
                // Placeholder for now
                format!("hashed_{}", p)
            })
        } else {
            None
        };

        Self {
            title,
            description,
            favicon_url,
            og_image,
            domain,
            is_safe: true, // Will be set by security scanner
            tags: request.tags.clone(),
            password_hash,
        }
    }
}

/// Metadata extracted from URL
#[derive(Debug, Clone)]
pub struct ExtractedMetadata {
    pub title: Option<String>,
    pub description: Option<String>,
    pub favicon_url: Option<String>,
    pub og_image: Option<String>,
}

// =============================================================================
// QUERY FILTERS
// =============================================================================

/// Pagination parameters
#[derive(Debug, Clone, Deserialize, ToSchema, IntoParams)]
#[schema(example = json!({
    "page": 1,
    "per_page": 20
}))]
pub struct LinkPagination {
    #[serde(default = "default_page")]
    pub page: i64,

    #[serde(default = "default_per_page")]
    pub per_page: i64,
}

fn default_page() -> i64 {
    1
}
fn default_per_page() -> i64 {
    20
}

impl Default for LinkPagination {
    fn default() -> Self {
        Self {
            page: default_page(),
            per_page: default_per_page(),
        }
    }
}

impl LinkPagination {
    pub fn offset(&self) -> i64 {
        (self.page - 1) * self.per_page
    }

    pub fn limit(&self) -> i64 {
        self.per_page.min(100) // Max 100 per page
    }
}

/// Link filter parameters
#[derive(Debug, Clone, Deserialize, Default, ToSchema, IntoParams)]
#[schema(example = json!({
    "search": "example",
    "tags": ["work", "important"],
    "is_active": true,
    "has_password": false,
    "domain": "example.com",
    "created_after": "2024-01-01T00:00:00Z",
    "created_before": "2024-12-31T23:59:59Z"
}))]
pub struct LinkFilter {
    pub search: Option<String>,
    pub tags: Option<Vec<String>>,
    pub is_active: Option<bool>,
    pub has_password: Option<bool>,
    pub domain: Option<String>,
    pub created_after: Option<DateTime<Utc>>,
    pub created_before: Option<DateTime<Utc>>,
}

/// Link list response
#[derive(Debug, Clone, Serialize, ToSchema)]
#[schema(example = json!({
    "links": [
        {
            "id": "123e4567-e89b-12d3-a456-426614174000",
            "short_code": "abc123",
            "original_url": "https://example.com/very/long/url",
            "short_url": "https://qck.sh/r/abc123",
            "title": "Example Site",
            "description": "A great example website",
            "expires_at": null,
            "created_at": "2024-01-01T12:00:00Z",
            "updated_at": "2024-01-01T12:00:00Z",
            "is_active": true,
            "tags": ["example", "test"],
            "is_password_protected": false
        }
    ],
    "total": 100,
    "page": 1,
    "per_page": 20,
    "total_pages": 5
}))]
pub struct LinkListResponse {
    pub links: Vec<LinkResponse>,
    pub total: i64,
    pub page: i64,
    pub per_page: i64,
    pub total_pages: i64,
}

// =============================================================================
// HELPER FUNCTIONS
// =============================================================================

/// Extract domain from URL
fn extract_domain(url: &str) -> Option<String> {
    url::Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(String::from))
}

/// Convert Link model to LinkResponse
impl Link {
    pub fn to_response(&self, base_url: &str) -> LinkResponse {
        // Create default stats for when no ClickHouse data is available
        let default_stats = LinkClickStats::default();
        self.to_response_with_stats(base_url, default_stats)
    }

    pub fn to_response_with_stats(&self, base_url: &str, stats: LinkClickStats) -> LinkResponse {
        // Build metadata from actual database fields
        let domain = extract_domain(&self.original_url).unwrap_or_default();

        // Convert tags from Option<Vec<Option<String>>> to Vec<String>
        let tags: Vec<String> = self
            .tags
            .as_ref()
            .map(|t| t.iter().filter_map(|tag| tag.clone()).collect())
            .unwrap_or_default();

        let metadata = LinkMetadata {
            title: self.title.clone(),
            description: self.description.clone(),
            favicon_url: self.favicon_url.clone(),
            og_image: self.og_image.clone(),
            domain,
            is_safe: true, // Will be set by security scanner
            tags: tags.clone(),
            password_hash: self.password_hash.clone(),
        };

        LinkResponse {
            id: self.id,
            short_code: self.short_code.clone(),
            original_url: self.original_url.clone(),
            short_url: format!("{}/{}", base_url, self.short_code), // Clean URL: qck.sh/abc123
            title: self.title.clone(),
            description: self.description.clone(),
            expires_at: self.expires_at,
            created_at: self.created_at,
            updated_at: self.updated_at,
            is_active: self.is_active,
            tags,
            is_password_protected: self.password_hash.is_some(),
            metadata,
            total_clicks: stats.total_clicks,
            unique_visitors: stats.unique_visitors,
            bot_clicks: stats.bot_clicks,
            last_accessed_at: stats.last_accessed_at.or(self.last_accessed_at),
        }
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_custom_alias_validation() {
        let valid_aliases = vec!["abc123", "test-link", "my_link", "Link2024"];

        for alias in valid_aliases {
            assert!(CUSTOM_ALIAS_REGEX.is_match(alias), "Failed for: {}", alias);
        }

        let invalid_aliases = vec![
            "-start-with-dash",
            "_start_with_underscore",
            "has space",
            "has@special",
            "",
        ];

        for alias in invalid_aliases {
            assert!(
                !CUSTOM_ALIAS_REGEX.is_match(alias),
                "Should fail for: {}",
                alias
            );
        }
    }

    #[test]
    fn test_pagination() {
        let pagination = LinkPagination {
            page: 3,
            per_page: 20,
        };
        assert_eq!(pagination.offset(), 40);
        assert_eq!(pagination.limit(), 20);

        let large_pagination = LinkPagination {
            page: 1,
            per_page: 200,
        };
        assert_eq!(large_pagination.limit(), 100); // Should cap at 100
    }

    #[test]
    fn test_extract_domain() {
        assert_eq!(
            extract_domain("https://example.com/path"),
            Some("example.com".to_string())
        );
        assert_eq!(
            extract_domain("http://sub.domain.com"),
            Some("sub.domain.com".to_string())
        );
        assert_eq!(extract_domain("invalid-url"), None);
    }
}
