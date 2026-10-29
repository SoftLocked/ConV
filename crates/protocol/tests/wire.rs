use conv_protocol::{
    replication::Replica,
    sync::Message,
    wire::{FRAME_LIMIT, Reassembler, WireError, encode},
};

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
