use axum_test::TestServer;
use backend::alerts::types::{
    ActivePeriod, AffectedEntity, AlertCollection, AlertDetails, AlertFormat, AlertSection,
    AlertTranslation, MtaAlert, MtaAlertData,
};
use backend::source::Source;
use chrono::{Duration, Utc};
use serde_json::{Value, json};

#[sqlx::test]
async fn alerts_project_owned_collections_without_changing_wire_shape(pool: sqlx::PgPool) {
    let owners = crate::support::test_owners(pool.clone());
    let dataset = crate::support::mta_bus_dataset();
    let route_id = dataset.routes[0].id.clone();
    let stop_id = dataset.stops[0].id.clone();
    owners.network.persist(&dataset).await.unwrap();
    let now = chrono::DateTime::from_timestamp(Utc::now().timestamp(), 0).unwrap();
    let start = now - Duration::hours(2);
    let mut alert = MtaAlert {
        details: AlertDetails {
            id: uuid::Uuid::now_v7(),
            original_id: "projection".into(),
            created_at: start,
            updated_at: now,
            recorded_at: now,
            translations: vec![
                AlertTranslation {
                    section: AlertSection::Header,
                    format: AlertFormat::Plain,
                    language: "en".into(),
                    text: "Plain English".into(),
                },
                AlertTranslation {
                    section: AlertSection::Header,
                    format: AlertFormat::Html,
                    language: "es".into(),
                    text: "HTML Spanish".into(),
                },
                AlertTranslation {
                    section: AlertSection::Header,
                    format: AlertFormat::Html,
                    language: "en".into(),
                    text: "HTML English".into(),
                },
                AlertTranslation {
                    section: AlertSection::Description,
                    format: AlertFormat::Plain,
                    language: "en".into(),
                    text: "Description".into(),
                },
            ],
            active_periods: vec![
                ActivePeriod {
                    start_time: start,
                    end_time: Some(now + Duration::hours(1)),
                },
                ActivePeriod {
                    start_time: now - Duration::hours(1),
                    end_time: None,
                },
            ],
            entities: vec![
                AffectedEntity {
                    route_id: Some(route_id.clone()),
                    stop_id: Some(stop_id.clone()),
                    sort_order: 3,
                },
                AffectedEntity {
                    route_id: None,
                    stop_id: Some(stop_id.clone()),
                    sort_order: 0,
                },
                AffectedEntity {
                    route_id: Some(route_id.clone()),
                    stop_id: None,
                    sort_order: 1,
                },
            ],
        },
        data: MtaAlertData {
            display_before_active: 30,
            alert_type: "detour".into(),
            clone_id: None,
        },
    };
    owners
        .alerts
        .save_all(AlertCollection::MtaBus(vec![alert.clone()]))
        .await
        .unwrap();
    let retained = owners
        .alerts
        .get_all(Source::MtaBus, Some(now))
        .await
        .unwrap();
    assert_eq!(retained[0].translations.len(), 4);
    assert_eq!(retained[0].active_periods.len(), 2);
    assert_eq!(retained[0].entities.len(), 3);
    let (router, _) =
        backend::api::router(crate::support::app_state_from_owners(pool, owners.clone()))
            .split_for_parts();
    let server = TestServer::new(router);
    for path in [
        "/alerts/mta_bus".to_owned(),
        format!("/alerts/mta_bus?at={}", now.timestamp()),
    ] {
        let response = server.get(&path).await;
        response.assert_status_ok();
        let body: Value = response.json();
        let wire = &body[0];
        assert_eq!(wire["id"], alert.details.id.to_string());
        assert_eq!(
            wire["data"],
            json!({"source": "mta_bus", "display_before_active": 30, "alert_type": "detour", "clone_id": null})
        );
        assert_eq!(wire["start_time"], json!(start));
        assert!(wire.get("end_time").is_none());
        assert!(wire.get("recorded_at").is_none());
        assert!(wire.get("active_periods").is_none());
        assert_eq!(
            wire["translations"],
            json!([
                {"section": "header", "format": "html", "language": "en", "text": "HTML English"},
                {"section": "description", "format": "plain", "language": "en", "text": "Description"}
            ])
        );
        assert_eq!(
            wire["entities"],
            json!([
                {"route_id": route_id, "sort_order": 1},
                {"route_id": route_id, "sort_order": 3, "stop_id": stop_id}
            ])
        );
    }
    alert.details.active_periods[1].end_time = Some(now + Duration::hours(3));
    owners
        .alerts
        .save_all(AlertCollection::MtaBus(vec![alert]))
        .await
        .unwrap();
    let ended: Value = server.get("/alerts/mta_bus").await.json();
    assert_eq!(ended[0]["end_time"], json!(now + Duration::hours(3)));
}
