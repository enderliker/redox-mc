use redox_mc_abi::types::{NETWORK_TARGET_BROADCAST, NETWORK_TARGET_PLAYER, NETWORK_TARGET_SERVER};
use redox_mc_api::network::{
    clear_outbox, current_side, dispatch_received, drain_outbox, drain_outbox_for_side, outbox_len,
    outbox_len_for_side, set_callback_side, set_running_side, NetworkChannel, NetworkContext,
    NetworkError, NetworkMessage, NetworkSide, PlayerId, SenderId, MAX_MESSAGE_SIZE,
    MAX_SERVERBOUND_MESSAGE_SIZE,
};
use std::sync::{Arc, Mutex, MutexGuard};

/// Serializes tests that share the process-wide outbox.
static OUTBOX_LOCK: Mutex<()> = Mutex::new(());

fn outbox_guard() -> MutexGuard<'static, ()> {
    OUTBOX_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Ping {
    nonce: u64,
    label: String,
}

impl NetworkMessage for Ping {
    fn encode(&self) -> Result<Vec<u8>, NetworkError> {
        let mut data = self.nonce.to_le_bytes().to_vec();
        data.extend_from_slice(self.label.as_bytes());
        Ok(data)
    }

    fn decode(bytes: &[u8]) -> Result<Self, NetworkError> {
        if bytes.len() < 8 {
            return Err(NetworkError::Decode("ping needs at least 8 bytes".into()));
        }
        let nonce = u64::from_le_bytes(bytes[..8].try_into().unwrap());
        let label = String::from_utf8(bytes[8..].to_vec())
            .map_err(|err| NetworkError::Decode(err.to_string()))?;
        Ok(Self { nonce, label })
    }
}

#[test]
fn valid_channel_identifiers_are_accepted() {
    for id in [
        "testmod:data",
        "test-mod:path/to/data",
        "a:b",
        "mod_1:some.message-v2",
    ] {
        let channel = NetworkChannel::<Vec<u8>>::register(id)
            .unwrap_or_else(|err| panic!("channel '{id}' should be valid: {err}"));
        assert_eq!(channel.id(), id);
    }
}

#[test]
fn invalid_channel_identifiers_are_rejected() {
    for id in [
        "",
        "missing_separator",
        ":path",
        "namespace:",
        "Name:path",
        "namespace:Path",
        "namespace:path:extra",
        "namespace:path/",
        "namespace:/path",
        "namespace:a//b",
        "minecraft:data",
        "realms:data",
        "redox:data",
    ] {
        match NetworkChannel::<Vec<u8>>::register(id) {
            Err(NetworkError::InvalidChannel(rejected)) => assert_eq!(rejected, id),
            other => panic!("channel '{id}' should be rejected, got {other:?}"),
        }
    }
}

#[test]
fn raw_bytes_round_trip() {
    let payload = b"\x00\x01\xFE\xFF raw".to_vec();
    let encoded = payload.encode().unwrap();
    assert_eq!(encoded, payload);
    assert_eq!(Vec::<u8>::decode(&encoded).unwrap(), payload);
}

#[test]
fn primitive_messages_round_trip() {
    assert_eq!(u32::decode(&42u32.encode().unwrap()).unwrap(), 42);
    assert_eq!(i64::decode(&(-7i64).encode().unwrap()).unwrap(), -7);
    assert!(bool::decode(&true.encode().unwrap()).unwrap());
    assert!(!bool::decode(&false.encode().unwrap()).unwrap());
    assert_eq!(
        String::decode(&"héllo".to_string().encode().unwrap()).unwrap(),
        "héllo"
    );
    assert_eq!(<()>::decode(&().encode().unwrap()).unwrap(), ());
    assert!(<()>::decode(&[0u8]).is_err());
    assert!(u16::decode(&[1u8]).is_err());
    assert!(bool::decode(&[2u8]).is_err());
}

#[test]
fn typed_dispatch_delivers_payload_and_sender_identity() {
    let received = Arc::new(Mutex::new(None::<(Ping, NetworkContext)>));
    let sink = Arc::clone(&received);

    NetworkChannel::<Ping>::register("testmod:typed")
        .unwrap()
        .on_receive(move |ctx: &NetworkContext, ping: Ping| {
            *sink.lock().unwrap() = Some((ping, ctx.clone()));
        });

    let ping = Ping {
        nonce: 0xDEAD_BEEF,
        label: "hello".to_string(),
    };
    let dispatched = dispatch_received(
        "testmod:typed",
        &ping.encode().unwrap(),
        "TestPlayer",
        "11111111-1111-1111-1111-111111111111",
        NetworkSide::Server,
    );

    assert_eq!(dispatched, 1);
    let (decoded, context) = received.lock().unwrap().take().expect("callback ran");
    assert_eq!(decoded, ping);
    assert_eq!(context.channel(), "testmod:typed");
    assert_eq!(context.sender().name(), "TestPlayer");
    assert_eq!(
        context.sender().uuid(),
        "11111111-1111-1111-1111-111111111111"
    );
    assert_eq!(context.side(), NetworkSide::Server);
    assert!(!context.sender().is_server());
    assert_eq!(
        context.player(),
        Some(PlayerId::new(
            "TestPlayer",
            "11111111-1111-1111-1111-111111111111"
        ))
    );
}

#[test]
fn unknown_channel_and_malformed_payload_are_ignored() {
    let calls = Arc::new(Mutex::new(0usize));
    let sink = Arc::clone(&calls);

    NetworkChannel::<Ping>::register("testmod:malformed")
        .unwrap()
        .on_receive(move |_ctx, _ping| {
            *sink.lock().unwrap() += 1;
        });

    assert_eq!(
        dispatch_received(
            "testmod:unknown",
            b"whatever",
            "player",
            "",
            NetworkSide::Server
        ),
        0
    );

    // Malformed payloads are logged and never reach the typed callback.
    assert_eq!(
        dispatch_received(
            "testmod:malformed",
            b"short",
            "player",
            "",
            NetworkSide::Server
        ),
        1
    );
    assert_eq!(*calls.lock().unwrap(), 0);
}

#[test]
fn every_registered_handler_runs() {
    let calls = Arc::new(Mutex::new(Vec::new()));

    for index in 0..3 {
        let sink = Arc::clone(&calls);
        NetworkChannel::<Vec<u8>>::register("testmod:multi")
            .unwrap()
            .on_receive(move |_ctx, data: Vec<u8>| {
                sink.lock().unwrap().push((index, data));
            });
    }

    assert_eq!(
        dispatch_received("testmod:multi", b"payload", "p", "u", NetworkSide::Client),
        3
    );

    let mut calls = calls.lock().unwrap().clone();
    calls.sort_by_key(|(index, _)| *index);
    assert_eq!(calls.len(), 3);
    for (index, data) in calls {
        assert_eq!(data, b"payload");
        assert!(index < 3);
    }
}

#[test]
fn sender_helpers_distinguish_server_from_player() {
    let server = SenderId::server();
    assert!(server.is_server());
    assert_eq!(server.name(), "server");
    assert_eq!(server.player_id(), None);

    // A player literally named "server" must not be mistaken for the server.
    let player_named_server = SenderId::new("server", "");
    assert!(!player_named_server.is_server());
    assert_eq!(
        player_named_server.player_id(),
        Some(PlayerId::new("server", ""))
    );

    let player = SenderId::new("Alex", "uuid");
    assert!(!player.is_server());
    assert_eq!(player.player_id(), Some(PlayerId::new("Alex", "uuid")));

    let unknown = SenderId::new("", "");
    assert!(!unknown.is_server());
    assert_eq!(unknown.player_id(), None);
}

#[test]
fn current_side_prefers_the_callback_scope() {
    set_running_side(NetworkSide::Server);
    set_callback_side(Some(NetworkSide::Client));
    assert_eq!(current_side(), NetworkSide::Client);

    set_callback_side(None);
    assert_eq!(current_side(), NetworkSide::Server);
}

#[test]
fn outbound_messages_are_routed_by_destination() {
    let _guard = outbox_guard();
    clear_outbox();
    set_running_side(NetworkSide::Server);
    set_callback_side(None);

    let channel = NetworkChannel::<Ping>::register("testmod:outbound").unwrap();
    let ping = Ping {
        nonce: 7,
        label: "out".to_string(),
    };
    let player = PlayerId::new("TestPlayer", "uuid-1");

    channel.send(&ping).unwrap();
    channel.send_to_player(&player, &ping).unwrap();
    channel.send_to_server(&ping).unwrap();
    assert_eq!(outbox_len(), 3);
    assert_eq!(outbox_len_for_side(NetworkSide::Server), 2);
    assert_eq!(outbox_len_for_side(NetworkSide::Client), 1);

    let mut seen = Vec::new();
    drain_outbox(|channel, target_kind, target_name, target_uuid, data| {
        seen.push((
            channel.to_string(),
            target_kind,
            target_name.to_string(),
            target_uuid.to_string(),
            data.to_vec(),
        ));
        true
    });

    assert_eq!(outbox_len(), 0);
    assert_eq!(seen.len(), 3);
    assert_eq!(seen[0].0, "testmod:outbound");
    assert_eq!(seen[0].1, NETWORK_TARGET_BROADCAST);
    assert_eq!(seen[1].1, NETWORK_TARGET_PLAYER);
    assert_eq!(seen[1].2, "TestPlayer");
    assert_eq!(seen[1].3, "uuid-1");
    assert_eq!(seen[2].1, NETWORK_TARGET_SERVER);
    assert_eq!(seen[0].4, ping.encode().unwrap());
    assert_eq!(seen[2].4, ping.encode().unwrap());
}

#[test]
fn undeliverable_messages_do_not_block_the_queue() {
    let _guard = outbox_guard();
    clear_outbox();
    set_running_side(NetworkSide::Server);
    set_callback_side(None);

    let channel = NetworkChannel::<u32>::register("testmod:drain").unwrap();
    channel.send(&1).unwrap();
    channel.send(&2).unwrap();

    let mut visited = 0;
    drain_outbox(|_channel, _kind, _name, _uuid, _data| {
        visited += 1;
        false
    });

    assert_eq!(visited, 2);
    assert_eq!(outbox_len(), 0);
}

#[test]
fn oversized_messages_are_rejected() {
    let _guard = outbox_guard();
    clear_outbox();
    set_running_side(NetworkSide::Server);
    set_callback_side(None);

    let channel = NetworkChannel::<Vec<u8>>::register("testmod:large").unwrap();
    let oversized = vec![0u8; MAX_MESSAGE_SIZE + 1];
    match channel.send(&oversized) {
        Err(NetworkError::MessageTooLarge { size, max }) => {
            assert_eq!(size, MAX_MESSAGE_SIZE + 1);
            assert_eq!(max, MAX_MESSAGE_SIZE);
        }
        other => panic!("expected MessageTooLarge, got {other:?}"),
    }
    assert_eq!(outbox_len(), 0);
}

#[test]
fn oversized_serverbound_messages_are_rejected() {
    let _guard = outbox_guard();
    clear_outbox();

    let channel = NetworkChannel::<Vec<u8>>::register("testmod:large-client").unwrap();
    let oversized = vec![0u8; MAX_SERVERBOUND_MESSAGE_SIZE + 1];
    match channel.send_to_server(&oversized) {
        Err(NetworkError::MessageTooLarge { size, max }) => {
            assert_eq!(size, MAX_SERVERBOUND_MESSAGE_SIZE + 1);
            assert_eq!(max, MAX_SERVERBOUND_MESSAGE_SIZE);
        }
        other => panic!("expected MessageTooLarge, got {other:?}"),
    }

    // The same payload is legal in the clientbound direction.
    let player = PlayerId::new("TestPlayer", "uuid-1");
    channel.send_to_player(&player, &oversized).unwrap();
    assert_eq!(outbox_len_for_side(NetworkSide::Server), 1);
    clear_outbox();
}

#[test]
fn drain_outbox_for_side_leaves_the_other_side_queued() {
    let _guard = outbox_guard();
    clear_outbox();

    let channel = NetworkChannel::<u32>::register("testmod:side-drain").unwrap();
    let player = PlayerId::new("TestPlayer", "uuid-1");
    channel.send_to_player(&player, &1).unwrap();
    channel.send_to_server(&2).unwrap();
    channel.send_to_player(&player, &3).unwrap();

    let mut server_seen = Vec::new();
    drain_outbox_for_side(NetworkSide::Server, |_channel, kind, _name, _uuid, data| {
        server_seen.push((kind, data.to_vec()));
        true
    });

    assert_eq!(server_seen.len(), 2);
    assert_eq!(server_seen[0].1, 1u32.to_le_bytes().to_vec());
    assert_eq!(server_seen[1].1, 3u32.to_le_bytes().to_vec());
    assert_eq!(outbox_len_for_side(NetworkSide::Server), 0);
    assert_eq!(outbox_len_for_side(NetworkSide::Client), 1);

    let mut client_seen = Vec::new();
    drain_outbox_for_side(NetworkSide::Client, |_channel, kind, _name, _uuid, data| {
        client_seen.push((kind, data.to_vec()));
        true
    });

    assert_eq!(client_seen.len(), 1);
    assert_eq!(client_seen[0].0, NETWORK_TARGET_SERVER);
    assert_eq!(client_seen[0].1, 2u32.to_le_bytes().to_vec());
    assert_eq!(outbox_len(), 0);
}

#[test]
fn reply_routes_to_player_on_server_and_server_on_client() {
    let _guard = outbox_guard();
    clear_outbox();

    let channel = NetworkChannel::<u32>::register("testmod:reply").unwrap();

    let server_ctx = NetworkContext::new(
        "testmod:reply",
        SenderId::new("TestPlayer", "uuid-1"),
        NetworkSide::Server,
    );
    channel.reply(&server_ctx, &99).unwrap();

    let client_ctx = NetworkContext::new("testmod:reply", SenderId::server(), NetworkSide::Client);
    channel.reply(&client_ctx, &100).unwrap();

    let mut targets = Vec::new();
    drain_outbox(|_channel, target_kind, target_name, target_uuid, data| {
        targets.push((
            target_kind,
            target_name.to_string(),
            target_uuid.to_string(),
            data.to_vec(),
        ));
        true
    });

    assert_eq!(targets.len(), 2);
    assert_eq!(targets[0].0, NETWORK_TARGET_PLAYER);
    assert_eq!(targets[0].1, "TestPlayer");
    assert_eq!(targets[0].2, "uuid-1");
    assert_eq!(targets[0].3, 99u32.to_le_bytes().to_vec());
    assert_eq!(targets[1].0, NETWORK_TARGET_SERVER);
    assert_eq!(targets[1].3, 100u32.to_le_bytes().to_vec());
}

#[test]
fn reply_without_a_player_target_fails_closed() {
    let _guard = outbox_guard();
    clear_outbox();

    let channel = NetworkChannel::<u32>::register("testmod:reply-no-target").unwrap();
    let server_ctx = NetworkContext::new(
        "testmod:reply-no-target",
        SenderId::server(),
        NetworkSide::Server,
    );

    assert!(matches!(
        channel.reply(&server_ctx, &1),
        Err(NetworkError::NoReplyTarget)
    ));
    assert_eq!(outbox_len(), 0);
}
