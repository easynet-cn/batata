mod common;

use base64::Engine;
use batata_consul_client::ConsulClient;

#[tokio::test]
#[ignore]
async fn test_event_fire_and_list() {
    let client = common::create_client();
    let event_name = format!("test-event-{}", common::test_id());

    // Fire an event
    let (event, _) = client
        .event_fire(&event_name, None, "", "", "", &common::w())
        .await
        .unwrap();
    assert!(event.id.is_some(), "fired event should have an ID");
    assert_eq!(event.name, event_name, "event name should match");

    // List events and verify our event is present
    let (events, _) = client.event_list(&event_name, &common::q()).await.unwrap();
    assert!(!events.is_empty(), "event list should not be empty");

    let found = events.iter().any(|e| e.name == event_name);
    assert!(found, "our event should appear in the list");
}

#[tokio::test]
#[ignore]
async fn test_event_filter_by_name() {
    let client = common::create_client();
    let id = common::test_id();
    let name_a = format!("evt-a-{}", id);
    let name_b = format!("evt-b-{}", id);

    // Fire two events with different names
    let (evt_a, _) = client
        .event_fire(&name_a, None, "", "", "", &common::w())
        .await
        .unwrap();
    assert_eq!(evt_a.name, name_a);

    let (evt_b, _) = client
        .event_fire(&name_b, None, "", "", "", &common::w())
        .await
        .unwrap();
    assert_eq!(evt_b.name, name_b);

    // Filter by name_a
    let (events_a, _) = client.event_list(&name_a, &common::q()).await.unwrap();
    assert!(!events_a.is_empty(), "should find events with name_a");
    for e in &events_a {
        assert_eq!(
            e.name, name_a,
            "filtered list should only contain name_a events"
        );
    }

    // Filter by name_b
    let (events_b, _) = client.event_list(&name_b, &common::q()).await.unwrap();
    assert!(!events_b.is_empty(), "should find events with name_b");
    for e in &events_b {
        assert_eq!(
            e.name, name_b,
            "filtered list should only contain name_b events"
        );
    }
}

#[tokio::test]
#[ignore]
async fn test_event_id_to_index() {
    let client = common::create_client();
    let event_name = format!("evt-idx-{}", common::test_id());

    let (event, _) = client
        .event_fire(&event_name, None, "", "", "", &common::w())
        .await
        .unwrap();
    let event_id = event.id.expect("event should have an ID");
    assert!(!event_id.is_empty(), "event ID should not be empty");

    let index = ConsulClient::event_id_to_index(&event_id);
    assert!(
        index > 0,
        "event_id_to_index should return a non-zero value, got {}",
        index
    );
}

#[tokio::test]
#[ignore]
async fn test_event_payload() {
    let client = common::create_client();
    let event_name = format!("evt-payload-{}", common::test_id());
    let payload = b"hello-event-payload";

    // Fire event with payload
    let (event, _) = client
        .event_fire(&event_name, Some(payload), "", "", "", &common::w())
        .await
        .unwrap();
    assert_eq!(event.name, event_name);

    // List and verify payload is present
    let (events, _) = client.event_list(&event_name, &common::q()).await.unwrap();
    assert!(!events.is_empty(), "should find the event");

    let our_event = events.iter().find(|e| e.name == event_name);
    assert!(our_event.is_some(), "our event should be in the list");

    let our_event = our_event.unwrap();
    assert!(our_event.payload.is_some(), "event should have a payload");

    // The payload is base64-encoded in the response
    let payload_b64 = our_event.payload.as_deref().unwrap();
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(payload_b64)
        .expect("payload should be valid base64");
    assert_eq!(decoded, payload, "decoded payload should match original");
}
