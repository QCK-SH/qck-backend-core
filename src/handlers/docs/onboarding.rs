// OpenAPI documentation for onboarding endpoints

use serde_json::json;

pub fn select_plan_endpoint() -> serde_json::Value {
    json!({
        "post": {
                "summary": "Select subscription plan",
                "description": "Select a subscription plan during onboarding (Free, Pro, or Enterprise)",
                "tags": ["Onboarding"],
                "security": [{"BearerAuth": []}],
                "requestBody": {
                    "required": true,
                    "content": {
                        "application/json": {
                            "schema": {
                                "$ref": "#/components/schemas/SelectPlanRequest"
                            },
                            "examples": {
                                "free_plan": {
                                    "value": {
                                        "plan": "free",
                                        "price": 0
                                    }
                                },
                                "pro_plan": {
                                    "value": {
                                        "plan": "pro",
                                        "price": 19
                                    }
                                },
                                "enterprise_plan": {
                                    "value": {
                                        "plan": "enterprise",
                                        "price": 49
                                    }
                                }
                            }
                        }
                    }
                },
                "responses": {
                    "200": {
                        "description": "Plan selected successfully",
                        "content": {
                            "application/json": {
                                "schema": {
                                    "$ref": "#/components/schemas/SelectPlanResponse"
                                },
                                "example": {
                                    "success": true,
                                    "message": "Free plan activated successfully!",
                                    "data": {
                                        "onboarding_status": "completed",
                                        "subscription_tier": "free",
                                        "requires_payment": false,
                                        "next_step": "dashboard"
                                    }
                                }
                            }
                        }
                    },
                    "400": {
                        "description": "Invalid plan selection",
                        "content": {
                            "application/json": {
                                "schema": {
                                    "$ref": "#/components/schemas/ErrorResponse"
                                },
                                "example": {
                                    "success": false,
                                    "message": "Invalid plan selection: basic with price $10",
                                    "data": null
                                }
                            }
                        }
                    },
                    "401": {
                        "description": "Unauthorized - Invalid or missing token",
                        "content": {
                            "application/json": {
                                "schema": {
                                    "$ref": "#/components/schemas/ErrorResponse"
                                }
                            }
                        }
                    },
                    "403": {
                        "description": "Forbidden - Email not verified",
                        "content": {
                            "application/json": {
                                "schema": {
                                    "$ref": "#/components/schemas/ErrorResponse"
                                },
                                "example": {
                                    "success": false,
                                    "message": "Please verify your email before selecting a plan",
                                    "data": null
                                }
                            }
                        }
                    },
                    "500": {
                        "description": "Internal server error",
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

pub fn onboarding_status_endpoint() -> serde_json::Value {
    json!({
        "get": {
            "summary": "Get onboarding status",
            "description": "Get the current user's onboarding progress and next steps",
            "tags": ["Onboarding"],
            "security": [{"BearerAuth": []}],
            "responses": {
                "200": {
                    "description": "Onboarding status retrieved successfully",
                    "content": {
                        "application/json": {
                            "schema": {
                                "$ref": "#/components/schemas/OnboardingStatusResponse"
                            },
                            "example": {
                                "success": true,
                                "data": {
                                    "onboarding_status": "plan_selected",
                                    "email_verified": true,
                                    "subscription_tier": "pro",
                                    "completed_steps": ["registered", "verified", "plan_selected"],
                                    "next_step": "payment"
                                }
                            }
                        }
                    }
                },
                "401": {
                    "description": "Unauthorized - Invalid or missing token",
                    "content": {
                        "application/json": {
                            "schema": {
                                "$ref": "#/components/schemas/ErrorResponse"
                            }
                        }
                    }
                },
                "500": {
                    "description": "Internal server error",
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

pub fn onboarding_schemas() -> serde_json::Value {
    json!({
        "SelectPlanRequest": {
            "type": "object",
            "required": ["plan", "price"],
            "properties": {
                "plan": {
                    "type": "string",
                    "enum": ["free", "pro", "enterprise"],
                    "description": "The subscription plan to select"
                },
                "price": {
                    "type": "integer",
                    "enum": [0, 19, 49],
                    "description": "The price of the plan (must match the plan type)"
                }
            }
        },
        "SelectPlanResponse": {
            "type": "object",
            "properties": {
                "success": {
                    "type": "boolean",
                    "description": "Whether the request was successful"
                },
                "message": {
                    "type": "string",
                    "description": "Response message"
                },
                "data": {
                    "type": "object",
                    "nullable": true,
                    "properties": {
                        "onboarding_status": {
                            "type": "string",
                            "description": "Current onboarding status after plan selection"
                        },
                        "subscription_tier": {
                            "type": "string",
                            "description": "Selected subscription tier"
                        },
                        "requires_payment": {
                            "type": "boolean",
                            "description": "Whether payment is required for this plan"
                        },
                        "next_step": {
                            "type": "string",
                            "description": "Next step in the onboarding flow (dashboard or payment)"
                        }
                    }
                }
            }
        },
        "OnboardingStatusResponse": {
            "type": "object",
            "properties": {
                "success": {
                    "type": "boolean",
                    "description": "Whether the request was successful"
                },
                "data": {
                    "type": "object",
                    "properties": {
                        "onboarding_status": {
                            "type": "string",
                            "enum": ["registered", "verified", "plan_selected", "payment_pending", "completed"],
                            "description": "Current onboarding status"
                        },
                        "email_verified": {
                            "type": "boolean",
                            "description": "Whether the user's email has been verified"
                        },
                        "subscription_tier": {
                            "type": "string",
                            "enum": ["free", "pro", "enterprise"],
                            "description": "Current subscription tier"
                        },
                        "completed_steps": {
                            "type": "array",
                            "items": {
                                "type": "string"
                            },
                            "description": "List of completed onboarding steps"
                        },
                        "next_step": {
                            "type": "string",
                            "description": "Next step the user should take"
                        }
                    }
                }
            }
        }
    })
}
