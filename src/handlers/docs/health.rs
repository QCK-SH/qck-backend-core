// Health check endpoints OpenAPI documentation

use serde_json::json;

/// Health check endpoint documentation
pub fn health_endpoint() -> serde_json::Value {
    json!({
        "get": {
            "tags": ["Health"],
            "summary": "Health check endpoint",
            "description": "Returns the health status of the service and its dependencies",
            "operationId": "healthCheck",
            "responses": {
                "200": {
                    "description": "Service is healthy",
                    "content": {
                        "application/json": {
                            "schema": {
                                "type": "object",
                                "properties": {
                                    "status": {
                                        "type": "string",
                                        "enum": ["healthy", "degraded"],
                                        "description": "Overall health status"
                                    },
                                    "service": {
                                        "type": "string",
                                        "description": "Service name"
                                    },
                                    "timestamp": {
                                        "type": "string",
                                        "format": "date-time",
                                        "description": "Health check timestamp"
                                    },
                                    "components": {
                                        "type": "object",
                                        "properties": {
                                            "postgresql": {
                                                "type": "object",
                                                "properties": {
                                                    "status": {
                                                        "type": "string",
                                                        "enum": ["healthy", "unhealthy"]
                                                    },
                                                    "max_connections": {
                                                        "type": "integer",
                                                        "nullable": true
                                                    },
                                                    "error": {
                                                        "type": "string",
                                                        "nullable": true
                                                    }
                                                }
                                            },
                                            "redis": {
                                                "type": "object",
                                                "properties": {
                                                    "status": {
                                                        "type": "string",
                                                        "enum": ["healthy", "unhealthy"]
                                                    },
                                                    "latency_ms": {
                                                        "type": "number",
                                                        "nullable": true
                                                    },
                                                    "active_connections": {
                                                        "type": "integer",
                                                        "nullable": true
                                                    },
                                                    "total_connections": {
                                                        "type": "integer",
                                                        "nullable": true
                                                    },
                                                    "error": {
                                                        "type": "string",
                                                        "nullable": true
                                                    }
                                                }
                                            },
                                            "clickhouse": {
                                                "type": "object",
                                                "properties": {
                                                    "status": {
                                                        "type": "string",
                                                        "enum": ["healthy", "unhealthy"]
                                                    },
                                                    "latency_ms": {
                                                        "type": "integer",
                                                        "nullable": true
                                                    },
                                                    "error": {
                                                        "type": "string",
                                                        "nullable": true
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                },
                "503": {
                    "description": "Service is degraded",
                    "content": {
                        "application/json": {
                            "schema": {
                                "$ref": "#/components/schemas/HealthResponse"
                            }
                        }
                    }
                }
            }
        }
    })
}
