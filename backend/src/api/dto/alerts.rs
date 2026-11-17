use crate::alerts::types::{AlertData, AlertTranslation};
use crate::operations::identity::AlertId;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// API response for alerts - flattened with all related data
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct ApiAlert {
    pub id: AlertId,
    pub original_id: String,
    pub data: AlertData,
    pub translations: Vec<AlertTranslation>,
    /// Unix timestamp in seconds.
    pub created_at: i64,
    /// Unix timestamp in seconds.
    pub updated_at: i64,
    /// Unix timestamp of the earliest provider-declared active period; absent when none is declared.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_time: Option<i64>,
    /// Unix timestamp in seconds. Absent without declared periods or while any period has no end.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_time: Option<i64>,
    /// Entities affected by alert
    pub entities: Vec<ApiAlertEntity>,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct ApiAlertEntity {
    /// Affected route ID
    #[schema(example = "A")]
    pub route_id: String,
    /// The priority of the alert for the entity in ascending order
    pub sort_order: i32,
    /// Affected stop ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_id: Option<String>,
}

impl From<&crate::alerts::types::Alert> for ApiAlert {
    fn from(alert: &crate::alerts::types::Alert) -> Self {
        let mut entities: Vec<_> = alert
            .entities
            .iter()
            .filter_map(|entity| {
                Some(ApiAlertEntity {
                    route_id: entity.route_id.clone()?,
                    sort_order: entity.sort_order,
                    stop_id: entity.stop_id.clone(),
                })
            })
            .collect();
        entities.sort_by_key(|entity| entity.sort_order);
        Self {
            id: alert.id,
            original_id: alert.original_id.clone(),
            data: alert.data.clone(),
            translations: [
                crate::alerts::types::AlertSection::Header,
                crate::alerts::types::AlertSection::Description,
            ]
            .into_iter()
            .filter_map(|section| {
                alert
                    .translations
                    .iter()
                    .filter(|t| t.section == section)
                    .min_by_key(|t| {
                        (
                            t.format != crate::alerts::types::AlertFormat::Html,
                            t.language != "en",
                        )
                    })
                    .cloned()
            })
            .collect(),
            created_at: alert.created_at.timestamp(),
            updated_at: alert.updated_at.timestamp(),
            start_time: alert
                .active_periods
                .iter()
                .map(|p| p.start_time)
                .min()
                .map(|start| start.timestamp()),
            end_time: if alert.active_periods.iter().any(|p| p.end_time.is_none()) {
                None
            } else {
                alert
                    .active_periods
                    .iter()
                    .filter_map(|p| p.end_time)
                    .max()
                    .map(|end| end.timestamp())
            },
            entities,
        }
    }
}
