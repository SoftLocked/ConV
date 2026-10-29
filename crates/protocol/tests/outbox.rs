use conv_protocol::{
    outbox::Outbox,
    replication::Replica,
    sync::{Effect, Message},
};
#[test]
fn control_bypasses_batching_and_small_edits_share_a_message() {
    let mut source = Replica::new(1);
    let mut outbox = Outbox::default();
    let first = source.insert(None, "a").unwrap().unwrap();
    let second = source.insert(None, "b").unwrap().unwrap();
    let ready = outbox.push(vec![
        Effect::Send {
            to: 2,
            message: Message::Mutation(first),
        },
        Effect::Send {
            to: 2,
            message: Message::Hello,
        },
        Effect::Send {
            to: 2,
            message: Message::Mutation(second),
        },
    ]);
    assert_eq!(ready.len(), 1);
    assert!(matches!(
        ready[0],
        Effect::Send {
            message: Message::Hello,
            ..
        }
    ));
    assert!(
        matches!(&outbox.flush()[0], Effect::Send { message: Message::Batch(items), .. } if items.len() == 2)
    );
    assert!(outbox.flush().is_empty());
}
