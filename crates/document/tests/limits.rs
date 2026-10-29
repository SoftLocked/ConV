use conv_document::{Document, DocumentError, ElementId, Insertion};
#[test]
fn pending_dependency_limit_releases_when_predecessor_arrives() {
    let parent = ElementId {
        replica: 2,
        sequence: 1,
        offset: 0,
    };
    let child = |sequence| Insertion {
        id: ElementId {
            replica: 1,
            sequence,
            offset: 0,
        },
        anchor: Some(parent),
        logical_time: 2,
        value: 'x',
    };
    let mut state = Document::fragment((1..=100_000).map(child), []).unwrap();
    let before = state.clone();
    let extra = Document::fragment([child(100_001)], []).unwrap();
    assert_eq!(
        state.merge(&extra),
        Err(DocumentError::TooManyPendingDependencies)
    );
    assert_eq!(state, before);
    state
        .merge(
            &Document::fragment(
                [Insertion {
                    id: parent,
                    anchor: None,
                    logical_time: 1,
                    value: 'p',
                }],
                [],
            )
            .unwrap(),
        )
        .unwrap();
    state.merge(&extra).unwrap();
    assert_eq!(state.text().len(), 100_002);
}
