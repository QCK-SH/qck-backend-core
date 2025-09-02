// Link management OpenAPI endpoint definitions
// DEV-66, DEV-114, DEV-108, DEV-95: Link CRUD OpenAPI documentation

use serde_json::json;

/// Create link endpoint definition
pub fn create_link_endpoint() -> serde_json::Value {
    json!({
        "post": {
            "tags": ["Links"],
            "summary": "Create a new short link",
            "description": "Creates a new shortened URL with optional custom alias and metadata. URLs undergo comprehensive security scanning including phishing detection, malware checks, and homograph attack detection. Requires authentication.",
            "operationId": "createLink",
            "security": [{"bearerAuth": []}],
            "requestBody": {
                "required": true,
                "content": {
                    "application/json": {
                        "schema": {
                            "$ref": "#/components/schemas/CreateLinkRequest"
                        },
                        "example": {
                            "url": "https://example.com/very/long/url/that/needs/shortening",
                            "custom_alias": "my-custom-link",
                            "title": "My Awesome Link",
                            "description": "This is a description of my link",
                            "expires_at": "2024-12-31T23:59:59Z",
                            "tags": ["work", "important"],
                            "is_password_protected": false
                        }
                    }
                }
            },
            "responses": {
                "201": {
                    "description": "Link created successfully",
                    "content": {
                        "application/json": {
                            "schema": {
                                "$ref": "#/components/schemas/LinkResponse"
                            }
                        }
                    }
                },
                "400": {
                    "description": "Bad request - validation failed",
                    "content": {
                        "application/json": {
                            "schema": {
                                "$ref": "#/components/schemas/LinkError"
                            },
                            "examples": {
                                "validation_error": {
                                    "value": {
                                        "success": false,
                                        "error": {
                                            "code": "VALIDATION_ERROR",
                                            "description": "Invalid URL format"
                                        },
                                        "message": "Invalid request data"
                                    }
                                },
                                "security_blocked": {
                                    "value": {
                                        "success": false,
                                        "error": {
                                            "code": "SECURITY_BLOCKED",
                                            "description": "URL blocked due to security threats detected (Risk: High, Score: 75)"
                                        },
                                        "message": "Security scan failed: Phishing pattern detected; Suspicious TLD detected"
                                    }
                                }
                            }
                        }
                    }
                },
                "401": {
                    "description": "Unauthorized - invalid or missing token",
                    "content": {
                        "application/json": {
                            "schema": {
                                "$ref": "#/components/schemas/AuthError"
                            }
                        }
                    }
                },
                "409": {
                    "description": "Conflict - custom alias already exists",
                    "content": {
                        "application/json": {
                            "schema": {
                                "$ref": "#/components/schemas/LinkError"
                            },
                            "example": {
                                "success": false,
                                "error": {
                                    "code": "ALIAS_EXISTS",
                                    "description": "Custom alias 'my-link' is already taken"
                                },
                                "message": "Alias already exists"
                            }
                        }
                    }
                },
                "429": {
                    "description": "Too many requests - rate limit exceeded",
                    "content": {
                        "application/json": {
                            "schema": {
                                "$ref": "#/components/schemas/LinkError"
                            }
                        }
                    }
                }
            }
        }
    })
}

/// List links endpoint definition
pub fn list_links_endpoint() -> serde_json::Value {
    json!({
        "get": {
            "tags": ["Links"],
            "summary": "List user's links",
            "description": "Retrieves a paginated list of user's links with optional filtering. Requires authentication.",
            "operationId": "listLinks",
            "security": [{"bearerAuth": []}],
            "parameters": [
                {
                    "name": "page",
                    "in": "query",
                    "description": "Page number (1-based)",
                    "required": false,
                    "schema": {
                        "type": "integer",
                        "default": 1,
                        "minimum": 1
                    }
                },
                {
                    "name": "per_page",
                    "in": "query",
                    "description": "Number of links per page (max 100)",
                    "required": false,
                    "schema": {
                        "type": "integer",
                        "default": 20,
                        "minimum": 1,
                        "maximum": 100
                    }
                },
                {
                    "name": "search",
                    "in": "query",
                    "description": "Search in link titles, descriptions, and URLs",
                    "required": false,
                    "schema": {
                        "type": "string"
                    }
                },
                {
                    "name": "tags",
                    "in": "query",
                    "description": "Filter by tags (comma-separated)",
                    "required": false,
                    "schema": {
                        "type": "array",
                        "items": {
                            "type": "string"
                        }
                    },
                    "style": "form",
                    "explode": false
                },
                {
                    "name": "is_active",
                    "in": "query",
                    "description": "Filter by active status",
                    "required": false,
                    "schema": {
                        "type": "boolean"
                    }
                },
                {
                    "name": "has_password",
                    "in": "query",
                    "description": "Filter by password protection status",
                    "required": false,
                    "schema": {
                        "type": "boolean"
                    }
                },
                {
                    "name": "domain",
                    "in": "query",
                    "description": "Filter by domain",
                    "required": false,
                    "schema": {
                        "type": "string"
                    }
                },
                {
                    "name": "created_after",
                    "in": "query",
                    "description": "Filter links created after this date",
                    "required": false,
                    "schema": {
                        "type": "string",
                        "format": "date-time"
                    }
                },
                {
                    "name": "created_before",
                    "in": "query",
                    "description": "Filter links created before this date",
                    "required": false,
                    "schema": {
                        "type": "string",
                        "format": "date-time"
                    }
                }
            ],
            "responses": {
                "200": {
                    "description": "Links retrieved successfully",
                    "content": {
                        "application/json": {
                            "schema": {
                                "$ref": "#/components/schemas/LinkListResponse"
                            }
                        }
                    }
                },
                "401": {
                    "description": "Unauthorized - invalid or missing token",
                    "content": {
                        "application/json": {
                            "schema": {
                                "$ref": "#/components/schemas/AuthError"
                            }
                        }
                    }
                }
            }
        }
    })
}

/// Get link endpoint definition
pub fn get_link_endpoint() -> serde_json::Value {
    json!({
        "get": {
            "tags": ["Links"],
            "summary": "Get a specific link",
            "description": "Retrieves details of a specific link by ID. Only the link owner can access it.",
            "operationId": "getLink",
            "security": [{"bearerAuth": []}],
            "parameters": [
                {
                    "name": "id",
                    "in": "path",
                    "description": "Link ID (UUID)",
                    "required": true,
                    "schema": {
                        "type": "string",
                        "format": "uuid"
                    },
                    "example": "123e4567-e89b-12d3-a456-426614174000"
                }
            ],
            "responses": {
                "200": {
                    "description": "Link retrieved successfully",
                    "content": {
                        "application/json": {
                            "schema": {
                                "$ref": "#/components/schemas/LinkResponse"
                            }
                        }
                    }
                },
                "401": {
                    "description": "Unauthorized - invalid or missing token",
                    "content": {
                        "application/json": {
                            "schema": {
                                "$ref": "#/components/schemas/AuthError"
                            }
                        }
                    }
                },
                "403": {
                    "description": "Forbidden - not the link owner",
                    "content": {
                        "application/json": {
                            "schema": {
                                "$ref": "#/components/schemas/LinkError"
                            }
                        }
                    }
                },
                "404": {
                    "description": "Link not found",
                    "content": {
                        "application/json": {
                            "schema": {
                                "$ref": "#/components/schemas/LinkError"
                            },
                            "example": {
                                "success": false,
                                "error": {
                                    "code": "NOT_FOUND",
                                    "description": "Link not found"
                                },
                                "message": "The requested link could not be found"
                            }
                        }
                    }
                }
            }
        }
    })
}

/// Update link endpoint definition
pub fn update_link_endpoint() -> serde_json::Value {
    json!({
        "put": {
            "tags": ["Links"],
            "summary": "Update an existing link",
            "description": "Updates an existing link's properties. Only the link owner can update it.",
            "operationId": "updateLink",
            "security": [{"bearerAuth": []}],
            "parameters": [
                {
                    "name": "id",
                    "in": "path",
                    "description": "Link ID (UUID)",
                    "required": true,
                    "schema": {
                        "type": "string",
                        "format": "uuid"
                    },
                    "example": "123e4567-e89b-12d3-a456-426614174000"
                }
            ],
            "requestBody": {
                "required": true,
                "content": {
                    "application/json": {
                        "schema": {
                            "$ref": "#/components/schemas/UpdateLinkRequest"
                        },
                        "example": {
                            "url": "https://updated-example.com/new/url",
                            "title": "Updated Title",
                            "description": "Updated description",
                            "expires_at": "2024-12-31T23:59:59Z",
                            "is_active": true,
                            "tags": ["updated", "modified"],
                            "is_password_protected": false
                        }
                    }
                }
            },
            "responses": {
                "200": {
                    "description": "Link updated successfully",
                    "content": {
                        "application/json": {
                            "schema": {
                                "$ref": "#/components/schemas/LinkResponse"
                            }
                        }
                    }
                },
                "400": {
                    "description": "Bad request - validation failed",
                    "content": {
                        "application/json": {
                            "schema": {
                                "$ref": "#/components/schemas/LinkError"
                            }
                        }
                    }
                },
                "401": {
                    "description": "Unauthorized - invalid or missing token",
                    "content": {
                        "application/json": {
                            "schema": {
                                "$ref": "#/components/schemas/AuthError"
                            }
                        }
                    }
                },
                "403": {
                    "description": "Forbidden - not the link owner",
                    "content": {
                        "application/json": {
                            "schema": {
                                "$ref": "#/components/schemas/LinkError"
                            }
                        }
                    }
                },
                "404": {
                    "description": "Link not found",
                    "content": {
                        "application/json": {
                            "schema": {
                                "$ref": "#/components/schemas/LinkError"
                            }
                        }
                    }
                }
            }
        }
    })
}

/// Delete link endpoint definition
pub fn delete_link_endpoint() -> serde_json::Value {
    json!({
        "delete": {
            "tags": ["Links"],
            "summary": "Delete (deactivate) a link",
            "description": "Soft deletes a link by deactivating it. The link will no longer be accessible but stats are preserved. Only the link owner can delete it.",
            "operationId": "deleteLink",
            "security": [{"bearerAuth": []}],
            "parameters": [
                {
                    "name": "id",
                    "in": "path",
                    "description": "Link ID (UUID)",
                    "required": true,
                    "schema": {
                        "type": "string",
                        "format": "uuid"
                    },
                    "example": "123e4567-e89b-12d3-a456-426614174000"
                }
            ],
            "responses": {
                "204": {
                    "description": "Link deleted successfully (no content)"
                },
                "401": {
                    "description": "Unauthorized - invalid or missing token",
                    "content": {
                        "application/json": {
                            "schema": {
                                "$ref": "#/components/schemas/AuthError"
                            }
                        }
                    }
                },
                "403": {
                    "description": "Forbidden - not the link owner",
                    "content": {
                        "application/json": {
                            "schema": {
                                "$ref": "#/components/schemas/LinkError"
                            }
                        }
                    }
                },
                "404": {
                    "description": "Link not found",
                    "content": {
                        "application/json": {
                            "schema": {
                                "$ref": "#/components/schemas/LinkError"
                            }
                        }
                    }
                }
            }
        }
    })
}

/// Get link stats endpoint definition
pub fn get_link_stats_endpoint() -> serde_json::Value {
    json!({
        "get": {
            "tags": ["Links"],
            "summary": "Get link statistics",
            "description": "Retrieves detailed statistics for a specific link including click count, creation date, and performance metrics. Only the link owner can access stats.",
            "operationId": "getLinkStats",
            "security": [{"bearerAuth": []}],
            "parameters": [
                {
                    "name": "id",
                    "in": "path",
                    "description": "Link ID (UUID)",
                    "required": true,
                    "schema": {
                        "type": "string",
                        "format": "uuid"
                    },
                    "example": "123e4567-e89b-12d3-a456-426614174000"
                }
            ],
            "responses": {
                "200": {
                    "description": "Link statistics retrieved successfully",
                    "content": {
                        "application/json": {
                            "schema": {
                                "$ref": "#/components/schemas/LinkStats"
                            },
                            "example": {
                                "short_code": "abc123",
                                "original_url": "https://example.com/very/long/url",
                                "total_clicks": 42,
                                "created_at": "2024-01-01T12:00:00Z",
                                "last_accessed_at": "2024-01-15T14:30:00Z",
                                "is_active": true,
                                "days_active": 15,
                                "average_clicks_per_day": 2.8
                            }
                        }
                    }
                },
                "401": {
                    "description": "Unauthorized - invalid or missing token",
                    "content": {
                        "application/json": {
                            "schema": {
                                "$ref": "#/components/schemas/AuthError"
                            }
                        }
                    }
                },
                "403": {
                    "description": "Forbidden - not the link owner",
                    "content": {
                        "application/json": {
                            "schema": {
                                "$ref": "#/components/schemas/LinkError"
                            }
                        }
                    }
                },
                "404": {
                    "description": "Link not found",
                    "content": {
                        "application/json": {
                            "schema": {
                                "$ref": "#/components/schemas/LinkError"
                            }
                        }
                    }
                }
            }
        }
    })
}

/// Bulk create links endpoint definition
pub fn bulk_create_links_endpoint() -> serde_json::Value {
    json!({
        "post": {
            "tags": ["Links"],
            "summary": "Bulk create multiple links",
            "description": "Creates multiple shortened URLs in a single request. Maximum 100 links per request. Returns success and failure counts with detailed results.",
            "operationId": "bulkCreateLinks",
            "security": [{"bearerAuth": []}],
            "requestBody": {
                "required": true,
                "content": {
                    "application/json": {
                        "schema": {
                            "type": "array",
                            "items": {
                                "$ref": "#/components/schemas/CreateLinkRequest"
                            },
                            "maxItems": 100,
                            "minItems": 1
                        },
                        "example": [
                            {
                                "url": "https://example1.com/url1",
                                "title": "First Link",
                                "tags": ["batch", "test"]
                            },
                            {
                                "url": "https://example2.com/url2",
                                "custom_alias": "my-second-link",
                                "title": "Second Link",
                                "tags": ["batch", "important"]
                            }
                        ]
                    }
                }
            },
            "responses": {
                "207": {
                    "description": "Multi-status - partial success (some links created, some failed)",
                    "content": {
                        "application/json": {
                            "schema": {
                                "$ref": "#/components/schemas/BulkCreateResponse"
                            },
                            "example": {
                                "success": 1,
                                "failed": 1,
                                "links": [
                                    {
                                        "id": "123e4567-e89b-12d3-a456-426614174000",
                                        "short_code": "abc123",
                                        "original_url": "https://example1.com/url1",
                                        "short_url": "https://qck.sh/r/abc123",
                                        "title": "First Link",
                                        "click_count": 0,
                                        "created_at": "2024-01-01T12:00:00Z",
                                        "is_active": true
                                    }
                                ],
                                "errors": [
                                    {
                                        "index": 1,
                                        "error": "Custom alias 'my-second-link' already exists"
                                    }
                                ]
                            }
                        }
                    }
                },
                "400": {
                    "description": "Bad request - invalid request format or too many links",
                    "content": {
                        "application/json": {
                            "schema": {
                                "$ref": "#/components/schemas/LinkError"
                            },
                            "example": {
                                "success": false,
                                "error": {
                                    "code": "BAD_REQUEST",
                                    "description": "Maximum 100 links can be created at once"
                                },
                                "message": "Request validation failed"
                            }
                        }
                    }
                },
                "401": {
                    "description": "Unauthorized - invalid or missing token",
                    "content": {
                        "application/json": {
                            "schema": {
                                "$ref": "#/components/schemas/AuthError"
                            }
                        }
                    }
                },
                "429": {
                    "description": "Too many requests - rate limit exceeded",
                    "content": {
                        "application/json": {
                            "schema": {
                                "$ref": "#/components/schemas/LinkError"
                            }
                        }
                    }
                }
            }
        }
    })
}

/// Link-specific schemas for OpenAPI
pub fn link_schemas() -> serde_json::Value {
    json!({
        "LinkError": {
            "type": "object",
            "properties": {
                "success": {
                    "type": "boolean",
                    "description": "Always false for errors"
                },
                "error": {
                    "type": "object",
                    "properties": {
                        "code": {
                            "type": "string",
                            "description": "Error code",
                            "enum": [
                                "VALIDATION_ERROR",
                                "NOT_FOUND",
                                "ALIAS_EXISTS",
                                "FORBIDDEN",
                                "BAD_REQUEST",
                                "DATABASE_ERROR",
                                "URL_INVALID",
                                "RATE_LIMITED",
                                "SECURITY_BLOCKED",
                                "EXPIRED",
                                "PASSWORD_REQUIRED",
                                "INVALID_PASSWORD",
                                "INACTIVE",
                                "UNAUTHORIZED",
                                "CACHE_ERROR",
                                "METADATA_EXTRACTION_ERROR",
                                "SUBSCRIPTION_LIMIT_EXCEEDED",
                                "INTERNAL_ERROR",
                                "SERVICE_UNAVAILABLE"
                            ]
                        },
                        "description": {
                            "type": "string",
                            "description": "Human-readable error description"
                        }
                    }
                },
                "message": {
                    "type": "string",
                    "description": "Error message"
                }
            },
            "example": {
                "success": false,
                "error": {
                    "code": "NOT_FOUND",
                    "description": "Link not found"
                },
                "message": "The requested link could not be found"
            }
        },
        "LinkStats": {
            "type": "object",
            "properties": {
                "short_code": {
                    "type": "string",
                    "description": "The short code for the link"
                },
                "original_url": {
                    "type": "string",
                    "format": "uri",
                    "description": "Original long URL"
                },
                "total_clicks": {
                    "type": "integer",
                    "description": "Total number of clicks"
                },
                "created_at": {
                    "type": "string",
                    "format": "date-time",
                    "description": "When the link was created"
                },
                "last_accessed_at": {
                    "type": "string",
                    "format": "date-time",
                    "description": "When the link was last accessed"
                },
                "is_active": {
                    "type": "boolean",
                    "description": "Whether the link is active"
                },
                "days_active": {
                    "type": "integer",
                    "description": "Number of days since creation"
                },
                "average_clicks_per_day": {
                    "type": "number",
                    "format": "float",
                    "description": "Average clicks per day"
                }
            },
            "example": {
                "short_code": "abc123",
                "original_url": "https://example.com/very/long/url",
                "total_clicks": 42,
                "created_at": "2024-01-01T12:00:00Z",
                "last_accessed_at": "2024-01-15T14:30:00Z",
                "is_active": true,
                "days_active": 15,
                "average_clicks_per_day": 2.8
            }
        },
        "BulkCreateResponse": {
            "type": "object",
            "properties": {
                "success": {
                    "type": "integer",
                    "description": "Number of successfully created links"
                },
                "failed": {
                    "type": "integer",
                    "description": "Number of failed link creations"
                },
                "links": {
                    "type": "array",
                    "description": "Successfully created links",
                    "items": {
                        "$ref": "#/components/schemas/LinkResponse"
                    }
                },
                "errors": {
                    "type": "array",
                    "description": "Errors for failed links",
                    "items": {
                        "type": "object",
                        "properties": {
                            "index": {
                                "type": "integer",
                                "description": "Index of the failed link in the request array"
                            },
                            "error": {
                                "type": "string",
                                "description": "Error message for this link"
                            }
                        }
                    }
                }
            },
            "example": {
                "success": 1,
                "failed": 1,
                "links": [],
                "errors": [
                    {
                        "index": 1,
                        "error": "Invalid URL format"
                    }
                ]
            }
        }
    })
}
