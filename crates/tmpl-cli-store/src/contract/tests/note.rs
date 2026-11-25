use crate::contract::{Body, NoteError, NoteId, Priority, Tag, Title};

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
fn note_id_deserialization_validates_too() {
    assert!(serde_json::from_str::<NoteId>("0").is_err());
    assert_eq!(serde_json::from_str::<NoteId>("3").unwrap().get(), 3);
}

#[test]
fn title_is_trimmed_and_must_be_one_non_empty_line() {
    assert_eq!("  Buy milk ".parse::<Title>().unwrap().as_str(), "Buy milk");
    // Edge whitespace is trimmed before the check, so a trailing newline or tab
    // is accepted and stored trimmed (as before the control-character check).
    assert_eq!("Buy milk\n".parse::<Title>().unwrap().as_str(), "Buy milk");
    assert_eq!("Buy milk\t".parse::<Title>().unwrap().as_str(), "Buy milk");
    assert_eq!("   ".parse::<Title>(), Err(NoteError::EmptyTitle));
    assert_eq!("a\tb".parse::<Title>(), Err(NoteError::TitleNotSingleLine));
    assert_eq!("a\nb".parse::<Title>(), Err(NoteError::TitleNotSingleLine));
    assert_eq!(
        "\x1b[31mred\x1b[0m".parse::<Title>(),
        Err(NoteError::TitleNotSingleLine)
    );
    assert_eq!(
        "bell\x07here".parse::<Title>(),
        Err(NoteError::TitleNotSingleLine)
    );
    assert_eq!(
        "del\x7fchar".parse::<Title>(),
        Err(NoteError::TitleNotSingleLine)
    );
    assert_eq!(
        "c1\u{009b}csi".parse::<Title>(),
        Err(NoteError::TitleNotSingleLine)
    );
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
fn invalid_values_are_shown_escaped_in_their_messages() {
    // A value can carry control bytes. Its message shows it in Debug form, so
    // ESC, line feed and tab are escaped rather than written to a terminal.
    let value = "a\u{1b}\n\tb";
    assert_eq!(
        value.parse::<NoteId>().unwrap_err().to_string(),
        "invalid note id \"a\\u{1b}\\n\\tb\": expected a positive integer such as 1"
    );
    assert_eq!(
        value.parse::<Tag>().unwrap_err().to_string(),
        "invalid tag \"a\\u{1b}\\n\\tb\": use 1-32 lowercase letters, digits or '-'"
    );
    assert_eq!(
        value.parse::<Priority>().unwrap_err().to_string(),
        "invalid priority \"a\\u{1b}\\n\\tb\": expected one of low, normal, high"
    );
}
