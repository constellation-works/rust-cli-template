use crate::app::note::rule::{ListFilter, NewNote, next_id, select};
use std::num::NonZeroUsize;
use time::OffsetDateTime;
use tmpl_cli_store::contract::{Note, NoteId, Priority, Tag};

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
    notes.iter().map(|note| note.id.get()).collect()
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

#[test]
fn the_next_id_is_one_past_the_highest_whatever_the_order() {
    let at = |id: u64| {
        NewNote {
            title: "t".parse().unwrap(),
            body: None,
            tags: Vec::new(),
            priority: Priority::Normal,
        }
        .into_note(NoteId::try_from(id).unwrap(), OffsetDateTime::UNIX_EPOCH)
    };
    assert_eq!(next_id(&[]), Some(NoteId::FIRST));
    assert_eq!(next_id(&[at(3), at(1), at(7)]).map(NoteId::get), Some(8));
    assert_eq!(next_id(&[at(NoteId::MAX.get())]), None);
}

#[test]
fn new_note_sorts_and_dedups_tags_and_keeps_the_body() {
    let new = NewNote {
        title: "t".parse().unwrap(),
        body: Some("text".parse().unwrap()),
        tags: vec![
            "b".parse().unwrap(),
            "a".parse().unwrap(),
            "b".parse().unwrap(),
        ],
        priority: Priority::Low,
    };
    let note = new.into_note(NoteId::FIRST, OffsetDateTime::UNIX_EPOCH);
    let tags: Vec<&str> = note.tags.iter().map(Tag::as_str).collect();
    assert_eq!(tags, ["a", "b"]);
    assert_eq!(note.body.as_deref(), Some("text"));
}
