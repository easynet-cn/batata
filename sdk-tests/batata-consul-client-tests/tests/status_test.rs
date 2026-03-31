mod common;

#[tokio::test]
async fn test_status_leader() {
    let client = common::create_client();

    let (leader, _) = client.status_leader(&common::q()).await.unwrap();
    assert!(!leader.is_empty(), "leader should not be empty");
    assert!(
        leader.contains(':'),
        "leader should be in host:port format, got '{}'",
        leader
    );
}

#[tokio::test]
async fn test_status_peers() {
    let client = common::create_client();

    let (peers, _) = client.status_peers(&common::q()).await.unwrap();
    assert!(!peers.is_empty(), "peers list should have at least 1 entry");
    assert!(
        peers.len() >= 1,
        "should have at least 1 peer, got {}",
        peers.len()
    );

    // Each peer should be in host:port format
    for peer in &peers {
        assert!(!peer.is_empty(), "peer address should not be empty");
        assert!(
            peer.contains(':'),
            "peer '{}' should be in host:port format",
            peer
        );
    }
}

#[tokio::test]
async fn test_status_leader_format() {
    let client = common::create_client();

    let (leader, _) = client.status_leader(&common::q()).await.unwrap();
    assert!(!leader.is_empty(), "leader should not be empty");

    // Verify IP:port format
    let parts: Vec<&str> = leader.rsplitn(2, ':').collect();
    assert_eq!(
        parts.len(),
        2,
        "leader '{}' should have exactly one colon separating host and port",
        leader
    );

    let port_str = parts[0];
    let host = parts[1];

    assert!(!host.is_empty(), "host part should not be empty");
    let port: u16 = port_str
        .parse()
        .unwrap_or_else(|_| panic!("port '{}' should be a valid u16", port_str));
    assert!(port > 0, "port should be positive");
}
