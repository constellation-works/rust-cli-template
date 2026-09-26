use crate::note::{Note, NoteId, Priority, Tag};
use crate::query::{ListFilter, select};
use std::num::NonZeroUsize;
use time::OffsetDateTime;

fn note(id: u64, tag: &str, priority: Priority) -> Note {
    Note {
        id: NoteId::try_from(id).unwrap(),
        title: format!("note {id}").parse().unwrap(),
        body: None,
        tags: vec![tag.parse::<Tag>().unwrap()],
        priority,
        created_at: OffsetDateTime::UNIX_EPOCH,
    }
}

fn notes() -> Vec<Note> {
    vec![
        note(1, "home", Priority::High),
        note(2, "work", Priority::High),
        note(3, "home", Priority::Low),
        note(4, "home", Priority::High),
    ]
}

fn ids(notes: &[Note]) -> Vec<u64> {
    notes.iter().map(|n| n.id.get()).collect()
}

#[test]
fn filters_combine_and_keep_the_stored_order() {
    let filter = ListFilter {
        tag: Some("home".parse().unwrap()),
        priority: Some(Priority::High),
        limit: None,
    };
    let listed = select(notes(), &filter);
    assert_eq!(ids(&listed.notes), [1, 4]);
    assert_eq!(listed.total, 2);
    assert!(!listed.truncated());
}

#[test]
fn the_limit_applies_after_the_filters_and_the_total_counts_every_match() {
    // STD-01 R33, R34.
    let filter = ListFilter {
        tag: Some("home".parse().unwrap()),
        priority: None,
        limit: NonZeroUsize::new(2),
    };
    let listed = select(notes(), &filter);
    assert_eq!(ids(&listed.notes), [1, 3]);
    assert_eq!(listed.total, 3);
    assert!(listed.truncated());
}

#[test]
fn no_filter_returns_everything() {
    let listed = select(notes(), &ListFilter::default());
    assert_eq!(listed.total, 4);
    assert_eq!(listed.notes.len(), 4);
}
