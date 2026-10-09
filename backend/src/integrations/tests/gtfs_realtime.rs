use super::*;

fn empty_feed() -> FeedFuture {
    Box::pin(async {
        let feed = FeedMessage {
            header: crate::feed::FeedHeader {
                gtfs_realtime_version: "2.0".into(),
                ..Default::default()
            },
            entity: vec![],
        };
        Ok(feed.encode_to_vec().into())
    })
}

#[tokio::test]
async fn complete_empty_feeds_are_successful_snapshots() {
    let feeds = fetch_feeds(vec![
        ("trips".into(), empty_feed()),
        ("positions".into(), empty_feed()),
    ])
    .await
    .unwrap();
    assert_eq!(feeds.len(), 2);
    assert!(feeds.iter().all(|feed| feed.entity.is_empty()));
}

#[tokio::test]
async fn a_failed_required_feed_rejects_the_partial_collection() {
    for failed_label in ["trips", "positions"] {
        let failed: FeedFuture = Box::pin(async { anyhow::bail!("provider unavailable") });
        let mut feeds = vec![(failed_label.to_owned(), failed)];
        feeds.push(("other".into(), empty_feed()));
        let error = fetch_feeds(feeds).await.unwrap_err();
        assert!(error.to_string().contains(failed_label));
        assert!(format!("{error:#}").contains("provider unavailable"));
    }
}

#[tokio::test]
async fn an_undecodable_required_feed_rejects_the_collection() {
    let invalid: FeedFuture = Box::pin(async { Ok(vec![0xff].into()) });
    let error = fetch_feeds(vec![
        ("trips".into(), empty_feed()),
        ("positions".into(), invalid),
    ])
    .await
    .unwrap_err();
    assert!(error.to_string().contains("decode feed positions"));
}
