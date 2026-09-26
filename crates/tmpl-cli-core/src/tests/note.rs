use crate::note::{Body, NewNote, NoteError, NoteId, Priority, Tag, Title};
use time::OffsetDateTime;

#[test]
fn note_id_rejects_zero_and_non_numbers() {
    assert_eq!("7".parse::<NoteId>().unwrap().get(), 7);
    assert!(matches!(
        "0".parse::<NoteId>(),
        Err(NoteError::InvalidId(_))
    ));
    assert!(matches!(
        "abc".parse::<NoteId>(),
        Err(NoteError::InvalidId(_))
    ));
    assert!(matches!(
        "-1".parse::<NoteId>(),
        Err(NoteError::InvalidId(_))
    ));
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
    assert_eq!(NoteId::next_after(&[]), NoteId::FIRST);
    assert_eq!(NoteId::next_after(&[at(3), at(1), at(7)]).get(), 8);
}

#[test]
fn note_id_deserialization_validates_too() {
    assert!(serde_json::from_str::<NoteId>("0").is_err());
    assert_eq!(serde_json::from_str::<NoteId>("3").unwrap().get(), 3);
}

#[test]
fn title_is_trimmed_and_must_be_one_non_empty_line() {
    assert_eq!("  Buy milk ".parse::<Title>().unwrap().as_str(), "Buy milk");
    assert_eq!("   ".parse::<Title>(), Err(NoteError::EmptyTitle));
    assert_eq!("a\tb".parse::<Title>(), Err(NoteError::TitleNotSingleLine));
    assert_eq!("a\nb".parse::<Title>(), Err(NoteError::TitleNotSingleLine));
    let long = "x".repeat(Title::MAX_CHARS + 1);
    assert_eq!(
        long.parse::<Title>(),
        Err(NoteError::TitleTooLong(Title::MAX_CHARS + 1))
    );
}

#[test]
fn tag_accepts_only_lowercase_kebab_labels() {
    assert!("follow-up".parse::<Tag>().is_ok());
    assert!("Home".parse::<Tag>().is_err());
    assert!("a b".parse::<Tag>().is_err());
    assert!("".parse::<Tag>().is_err());
    assert!("x".repeat(Tag::MAX_CHARS + 1).parse::<Tag>().is_err());
}

#[test]
fn priority_round_trips_through_its_canonical_names() {
    for name in Priority::NAMES {
        assert_eq!(name.parse::<Priority>().unwrap().as_str(), name);
    }
    assert!("urgent".parse::<Priority>().is_err());
    assert_eq!(serde_json::to_string(&Priority::High).unwrap(), "\"high\"");
}

#[test]
fn a_blank_body_is_refused_not_dropped() {
    // STD-01 R29: caller input is applied or rejected, never silently dropped.
    assert_eq!(" \n ".parse::<Body>(), Err(NoteError::EmptyBody));
    let body: Body = "two\nlines".parse().unwrap();
    assert_eq!(body.as_str(), "two\nlines");
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
