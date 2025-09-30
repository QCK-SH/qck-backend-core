// URL redirection endpoint documentation

use serde_json::json;

/// Documentation for GET /{short_code} - Redirect to original URL
pub fn redirect_endpoint() -> serde_json::Value {
    json!({
        "get": {
            "tags": ["Redirect"],
            "summary": "Redirect to original URL",
            "description": "Redirects the user to the original URL associated with the short code. Returns 308 Permanent Redirect if found, 404 if not found.",
            "operationId": "redirectToUrl",
            "parameters": [
                {
                    "name": "short_code",
                    "in": "path",
                    "description": "The short code to redirect",
                    "required": true,
                    "schema": {
                        "type": "string",
                        "minLength": 3,
                        "maxLength": 20,
                        "pattern": "^[a-zA-Z0-9]+$",
                        "example": "abc123"
                    }
                }
            ],
            "responses": {
                "308": {
                    "description": "Permanent redirect to the original URL",
                    "headers": {
                        "Location": {
                            "description": "The original URL to redirect to",
                            "schema": {
                                "type": "string",
                                "format": "uri",
                                "example": "https://www.example.com"
                            }
                        }
                    }
                },
                "404": {
                    "description": "Short code not found",
                    "content": {
                        "application/json": {
                            "schema": {
                                "$ref": "#/components/schemas/ErrorResponse"
                            },
                            "example": {
                                "success": false,
                                "error": {
                                    "code": "NOT_FOUND",
                                    "description": "Short URL not found"
                                },
                                "message": "Short URL not found"
                            }
                        }
                    }
                },
                "410": {
                    "description": "Link has expired",
                    "content": {
                        "application/json": {
                            "schema": {
                                "$ref": "#/components/schemas/ErrorResponse"
                            }
                        }
                    }
                }
            }
        }
    })
}

/// Documentation for GET /{short_code}/preview - Preview URL destination
pub fn preview_endpoint() -> serde_json::Value {
    json!({
        "get": {
            "tags": ["Redirect"],
            "summary": "Preview URL destination",
            "description": "Returns information about the destination URL without redirecting. Useful for security checks or preview displays.",
            "operationId": "previewUrl",
            "parameters": [
                {
                    "name": "short_code",
                    "in": "path",
                    "description": "The short code to preview",
                    "required": true,
                    "schema": {
                        "type": "string",
                        "minLength": 3,
                        "maxLength": 20,
                        "pattern": "^[a-zA-Z0-9]+$",
                        "example": "abc123"
                    }
                }
            ],
            "responses": {
                "200": {
                    "description": "Preview information for the short URL",
                    "content": {
                        "application/json": {
                            "schema": {
                                "$ref": "#/components/schemas/PreviewResponse"
                            },
                            "example": {
                                "short_code": "abc123",
                                "destination_url": "https://www.example.com",
                                "title": "Example Domain",
                                "description": "Example Domain homepage",
                                "is_safe": true,
                                "created_at": "2025-01-01T00:00:00Z",
                                "expires_at": null
                            }
                        }
                    }
                },
                "404": {
                    "description": "Short code not found",
                    "content": {
                        "application/json": {
                            "schema": {
                                "$ref": "#/components/schemas/ErrorResponse"
                            }
                        }
                    }
                }
            }
        }
    })
}

/// Preview response schema
pub fn preview_response_schema() -> serde_json::Value {
    json!({
        "PreviewResponse": {
            "type": "object",
            "required": ["short_code", "destination_url", "is_safe"],
            "properties": {
                "short_code": {
                    "type": "string",
                    "description": "The short code"
                },
                "destination_url": {
                    "type": "string",
                    "format": "uri",
                    "description": "The original URL that will be redirected to"
                },
                "title": {
                    "type": "string",
                    "nullable": true,
                    "description": "Page title if available"
                },
                "description": {
                    "type": "string",
                    "nullable": true,
                    "description": "Page description if available"
                },
                "is_safe": {
                    "type": "boolean",
                    "description": "Whether the URL is considered safe"
                },
                "created_at": {
                    "type": "string",
                    "format": "date-time",
                    "description": "When the short link was created"
                },
                "expires_at": {
                    "type": "string",
                    "format": "date-time",
                    "nullable": true,
                    "description": "When the link expires (if applicable)"
                }
            }
        }
    })
}