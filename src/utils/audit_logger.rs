// DEV-114: Audit logging for all CRUD operations
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tracing::{info, warn};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
pub enum AuditAction {
    LinkCreated,
    LinkRead,
    LinkUpdated,
    LinkDeleted,
    LinkPermanentlyDeleted,
    BulkLinksDeleted,
    BulkStatusUpdated,
    LinkAccessed,
    LinkExpired,
    LinkPasswordFailed,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AuditLog {
    pub id: Uuid,
    pub action: AuditAction,
    pub user_id: Uuid,
    pub resource_id: Option<String>,
    pub resource_type: String,
    pub details: Option<String>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub timestamp: DateTime<Utc>,
}

pub struct AuditLogger;

impl AuditLogger {
    /// Log an audit event for link operations
    pub async fn log_link_action(
        action: AuditAction,
        user_id: Uuid,
        link_id: Option<String>,
        details: Option<String>,
    ) {
        let audit_log = AuditLog {
            id: Uuid::new_v4(),
            action,
            user_id,
            resource_id: link_id,
            resource_type: "link".to_string(),
            details,
            ip_address: None, // Would be passed from request context
            user_agent: None, // Would be passed from request context
            timestamp: Utc::now(),
        };

        // Log to tracing system (in production, this would also write to database/queue)
        let json_log = serde_json::to_string(&audit_log).unwrap_or_else(|e| {
            warn!("Failed to serialize audit log: {}", e);
            format!("{:?}", audit_log)
        });

        info!(target: "audit", "{}", json_log);

        // TODO: In production, also write to:
        // - Database audit table
        // - Message queue for processing
        // - External audit service
    }

    /// Log bulk operations
    pub async fn log_bulk_action(
        action: AuditAction,
        user_id: Uuid,
        affected_ids: Vec<String>,
        details: Option<String>,
    ) {
        let audit_log = AuditLog {
            id: Uuid::new_v4(),
            action,
            user_id,
            resource_id: Some(format!("{} links", affected_ids.len())),
            resource_type: "bulk_links".to_string(),
            details: details.or_else(|| Some(format!("Affected IDs: {:?}", affected_ids))),
            ip_address: None,
            user_agent: None,
            timestamp: Utc::now(),
        };

        let json_log = serde_json::to_string(&audit_log).unwrap_or_else(|e| {
            warn!("Failed to serialize audit log: {}", e);
            format!("{:?}", audit_log)
        });

        info!(target: "audit", "{}", json_log);
    }
}
