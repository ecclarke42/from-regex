use super::*;
use crate as from_regex;

#[derive(Debug, Clone, PartialEq, Eq, FromRegex)]
#[from_regex(pattern = "abc(?P<named>def)")]
struct MyStruct {
    named: String,
}

lazy_static! {
    static ref MY_STRUCT: MyStruct = MyStruct {
        named: String::from("def")
    };
}

#[derive(Debug, Clone, PartialEq, Eq, FromRegex)]
enum FlatEnum {
    #[from_regex(pattern = "c")]
    Shorter,

    #[from_regex(pattern = "(?P<a>a)(?P<b>b)?c")]
    Capturing { a: String, b: Option<String> },

    #[from_regex(default)]
    Fallback,
}

lazy_static! {
    static ref FLAT_CAPTURED_FULL: FlatEnum = FlatEnum::Capturing {
        a: String::from("a"),
        b: Some(String::from("b")),
    };
    static ref FLAT_CAPTURED_PARTIAL: FlatEnum = FlatEnum::Capturing {
        a: String::from("a"),
        b: None
    };
}

/// Similar to `FlatEnum`, but uses `MatchMode::First`
#[derive(Debug, Clone, PartialEq, Eq, FromRegex)]
#[from_regex(match_mode = "first")]
enum SortedEnum {
    #[from_regex(pattern = "c")]
    Shorter,

    #[from_regex(pattern = "(?P<a>a)?(?P<b>b)?c")]
    Capturing {
        a: Option<String>,
        b: Option<String>,
    },
}

lazy_static! {
    static ref SORTED_CAPTURED_FULL: SortedEnum = SortedEnum::Capturing {
        a: Some(String::from("a")),
        b: Some(String::from("b")),
    };
}

#[derive(Debug, Clone, PartialEq, Eq, FromRegex)]
enum NestedEnum {
    #[from_regex(pattern = "(?P<a>a)(?P<b>b)?c")]
    Capturing { a: String, b: Option<String> },

    #[from_regex(transparent)]
    Nested(MyStruct),
}

lazy_static! {
    static ref NESTED_CAPTURED_FULL: NestedEnum = NestedEnum::Capturing {
        a: String::from("a"),
        b: Some(String::from("b")),
    };
    static ref NESTED_CAPTURED_PARTIAL: NestedEnum = NestedEnum::Capturing {
        a: String::from("a"),
        b: None,
    };
}

#[test]
fn matches() {
    assert_eq!(
        MyStruct::from_regex("abcdef").expect("Didn't match MyStruct"),
        MY_STRUCT.clone()
    );

    // Flat Enum
    assert_eq!(
        FlatEnum::from_regex("abc").expect("Didn't match FlatEnum"),
        FLAT_CAPTURED_FULL.clone()
    );
    assert_eq!(
        FlatEnum::from_regex("c").expect("Didn't match FlatEnum"),
        FlatEnum::Shorter
    );
    assert_eq!(
        FlatEnum::from_regex("something else").expect("Didn't match FlatEnum"),
        FlatEnum::Fallback
    );

    // Sorted Enum (still matches `Capturing` because matches must match the
    // entire input, however, search would use `Shorter` for this input since
    // it has higher priority)
    assert_eq!(
        SortedEnum::from_regex("abc").expect("Didn't match SortedEnum"),
        SORTED_CAPTURED_FULL.clone()
    );

    // Nested Enum
    assert_eq!(
        NestedEnum::from_regex("abc").expect("Didn't match NestedEnum"),
        NESTED_CAPTURED_FULL.clone()
    );
    // Should match the longer of the two (which is the nested match)
    assert_eq!(
        NestedEnum::from_regex("abcdef").expect("Didn't match NestedEnum"),
        NestedEnum::Nested(MY_STRUCT.clone())
    );
}

const SEARCH_TEXT: &str = "abcdef, abc, a c ac bc ba bc";

#[test]
fn searches() {
    assert_eq!(
        MyStruct::matches(SEARCH_TEXT),
        vec![MyStruct {
            named: String::from("def")
        }]
    );

    assert_eq!(
        FlatEnum::matches(SEARCH_TEXT),
        vec![
            FLAT_CAPTURED_FULL.clone(),
            FLAT_CAPTURED_FULL.clone(),
            FlatEnum::Shorter,
            FLAT_CAPTURED_PARTIAL.clone(),
            FlatEnum::Shorter,
            FlatEnum::Shorter
        ]
    );

    // For SortedEnum, all the `"c"` bits will get picked up by `Shorter` and
    // keep other variants from overlapping
    assert_eq!(
        SortedEnum::matches(SEARCH_TEXT),
        vec![SortedEnum::Shorter; 6]
    );

    assert_eq!(
        NestedEnum::matches(SEARCH_TEXT),
        vec![
            NestedEnum::Nested(MY_STRUCT.clone()),
            NESTED_CAPTURED_FULL.clone(),
            NESTED_CAPTURED_PARTIAL.clone()
        ]
    );
}
