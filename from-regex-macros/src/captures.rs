use std::collections::HashMap;

use lazy_static::lazy_static;
use regex::Regex;

pub type Groups<'a> = HashMap<&'a str, bool>;

// TODO: Check edges. Regex::capture_names might help, here, but it doesn't tell us if they're optional
const CAPTURE_GROUP_PATTERN: &str = r"[\(]*\?P<(?P<group>[A-z0-9_]+)>[^\)]*[\)]*(?P<optional>\?)?";
lazy_static! {
    static ref CAPTURE_GROUP_REGEX: Regex = Regex::new(CAPTURE_GROUP_PATTERN).unwrap();
}

pub fn from_regex_pattern(pat: &str) -> Groups {
    let groups = CAPTURE_GROUP_REGEX
        .captures_iter(pat)
        .map(|cap| {
            let name = cap.name("group").unwrap().as_str();
            let optional = cap.name("optional").is_some();
            (name, optional)
        })
        .collect::<HashMap<_, _>>();

    groups
}
