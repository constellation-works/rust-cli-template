use crate::command::input::{parsed, parsed_list, parsed_option};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Probe {
    #[serde(deserialize_with = "parsed")]
    one: u8,
    #[serde(default, deserialize_with = "parsed_option")]
    maybe: Option<u8>,
    #[serde(default, deserialize_with = "parsed_list")]
    many: Vec<u8>,
}

#[test]
fn a_text_field_is_parsed_by_its_type_and_refused_when_it_does_not_parse() {
    let probe: Probe =
        serde_json::from_value(serde_json::json!({"one": "7", "many": ["1", "2"]})).unwrap();
    assert_eq!((probe.one, probe.maybe, probe.many), (7, None, vec![1, 2]));
    let probe: Probe =
        serde_json::from_value(serde_json::json!({"one": "7", "maybe": null})).unwrap();
    assert_eq!(probe.maybe, None);
    for bad in [
        serde_json::json!({"one": "x"}),
        serde_json::json!({"one": "7", "maybe": "300"}),
        serde_json::json!({"one": "7", "many": ["1", "-"]}),
    ] {
        assert!(serde_json::from_value::<Probe>(bad).is_err());
    }
}
