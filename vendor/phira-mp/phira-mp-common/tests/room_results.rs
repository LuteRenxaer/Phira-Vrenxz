use phira_mp_common::{Message, RoomResultEntry};

#[test]
fn room_results_roundtrip() {
    let msg = Message::RoomResults {
        results: vec![
            RoomResultEntry {
                user_id: 1,
                user_name: "Alice".into(),
                score: 1200000,
                accuracy: 0.993,
                full_combo: true,
                max_combo: 1234,
                perfect: 800,
                good: 5,
                bad: 1,
                miss: 0,
                aborted: false,
            },
            RoomResultEntry {
                user_id: 2,
                user_name: "Bob".into(),
                score: 0,
                accuracy: 0.,
                full_combo: false,
                max_combo: 0,
                perfect: 0,
                good: 0,
                bad: 0,
                miss: 0,
                aborted: true,
            },
        ],
    };
    let mut buf = Vec::new();
    phira_mp_common::encode_packet(&msg, &mut buf);
    let decoded: Message = phira_mp_common::decode_packet(&buf).unwrap();
    match decoded {
        Message::RoomResults { results } => {
            assert_eq!(results.len(), 2);
            assert_eq!(results[0].user_name, "Alice");
            assert_eq!(results[0].score, 1200000);
            assert!((results[0].accuracy - 0.993).abs() < 1e-4);
            assert_eq!(results[0].full_combo, true);
            assert_eq!(results[1].aborted, true);
        }
        other => panic!("decoded to wrong variant: {other:?}"),
    }
}
