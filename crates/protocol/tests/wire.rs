use conv_protocol::{
    replication::Replica,
    sync::Message,
    wire::{FRAME_LIMIT, Reassembler, WireError, encode},
};

#[test]
fn complete_frames_pass_even_with_eight_incomplete_bulk_transfers() {
    let mut source = conv_protocol::replication::Replica::new(9);
    let mutation = source.insert(None, &"x".repeat(200)).unwrap().unwrap();
    let mut incoming = Reassembler::new(1);
    for id in 0..8 {
        let frames = encode(1, 9, id, &Message::Mutation(mutation.clone())).unwrap();
        assert!(frames.len() > 1);
        assert!(incoming.receive(0, &frames[0]).unwrap().is_none());
    }
    let hello = encode(1, 10, 99, &Message::Hello).unwrap();
    assert!(matches!(
        incoming.receive(0, &hello[0]),
        Ok(Some((10, Message::Hello)))
    ));
}

#[test]
fn reordered_chunks_and_duplicates_reconstruct_a_valid_document() {
    let mut writer = Replica::new(7);
    let mutation = writer.insert(None, &"🦀".repeat(300)).unwrap().unwrap();
    let frames = encode(9, 7, 1, &Message::Mutation(mutation)).unwrap();
    assert!(frames.len() > 1);
    assert!(frames.iter().all(|frame| frame.len() <= FRAME_LIMIT));
    let mut receiver = Reassembler::new(9);
    let mut replica = Replica::new(8);
    for frame in frames.iter().rev() {
        if let Some((sender, Message::Mutation(mutation))) = receiver.receive(0, frame).unwrap() {
            assert_eq!(sender, 7);
            replica.receive(&mutation).unwrap();
        }
    }
    assert_eq!(writer.document(), replica.document());
    for frame in &frames {
        receiver.receive(1, frame).unwrap();
    }
}

#[test]
fn rejects_cross_room_corruption_and_excessive_chunk_count() {
    let mut replica = Replica::new(1);
    let mutation = replica.insert(None, "hello").unwrap().unwrap();
    let mut frames = encode(9, 1, 2, &Message::Mutation(mutation)).unwrap();
    assert!(matches!(
        Reassembler::new(10).receive(0, &frames[0]),
        Err(WireError::WrongRoom)
    ));
    *frames[0].last_mut().unwrap() ^= 1;
    assert!(matches!(
        Reassembler::new(9).receive(0, &frames[0]),
        Err(WireError::Corrupt)
    ));
    frames[0][44..48].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(matches!(
        Reassembler::new(9).receive(0, &frames[0]),
        Err(WireError::InvalidFrame)
    ));
}

#[test]
fn oversized_request_counts_are_rejected_before_allocating_entries() {
    use conv_protocol::{Range, replication::SyncRequest};
    let message = Message::Request(vec![
        SyncRequest {
            replica: 1,
            range: Range { start: 1, end: 1 }
        };
        65
    ]);
    let frames = encode(1, 1, 1, &message).unwrap();
    let mut decoder = Reassembler::new(1);
    let mut rejected = false;
    for frame in frames {
        rejected |= matches!(decoder.receive(0, &frame), Err(WireError::InvalidMessage));
    }
    assert!(rejected);
}
