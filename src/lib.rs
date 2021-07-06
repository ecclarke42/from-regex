extern crate from_regex_macros;
pub use from_regex_macros::*;
pub use lazy_static::lazy_static;

pub use regex;
pub use regex::{Captures, Regex};
pub use std::str::FromStr;

pub trait FromRegex: Sized {
    type CustomError;
    fn from_regex(s: &str) -> Result<Self, FromRegexError<Self::CustomError>>;
}

// #[cfg(feature = "from_str")]
// impl<T: FromRegex> std::str::FromStr for T {
//     type Err = FromRegexError<T::CustomError>;
//     fn from_str(s: &str) -> Result<Self, Self::Err> {
//         T::from_regex(s)
//     }
// }

#[derive(Debug)]
pub enum FromRegexError<Err> {
    NoMatch,
    Custom(Err),
}

impl<Err> std::fmt::Display for FromRegexError<Err>
where
    Err: std::fmt::Display,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoMatch => write!(f, "No match found"),
            Self::Custom(err) => write!(f, "Custom: {}", err),
        }
    }
}

impl<Err> std::error::Error for FromRegexError<Err> where Err: std::error::Error {}

// TODO: more error things?
// pub enum Test {
//     A,
//     B,
// }

// lazy_static! {
//     static ref A_REGEX: Regex = Regex::new("a").unwrap();
//     static ref B_REGEX: Regex = Regex::new("b").unwrap();
// }
// impl Test {
//     fn from_a_capture(cap: regex::Captures) -> Option<Self> {
//         todo!()
//     }
//     // fn from_regex_as_a(s: &str) -> Option<Self> {
//     //     A_REGEX.captures(s).map(Self::from_a_capture).flatten()
//     // }
//     fn from_regex_as_a_exact(s: &str) -> Option<Self> {
//         match A_REGEX.captures(s) {
//             Some(cap) => {
//                 if cap[0].len() == s.len() {
//                     Self::from_a_capture(cap)
//                 } else {
//                     None
//                 }
//             }
//             None => None,
//         }
//     }

//     fn regex_capture_a_consuming(s: &mut String) -> Vec<Self> {
//         let mut offset = 0;
//         let (ranges, values) = A_REGEX
//             .captures_iter(&s)
//             .filter_map(|cap| {
//                 let range = cap.get(0).unwrap().range(); // Unwrap should be fine, since otherwise it wouldn't match
//                 let value = Self::from_a_capture(cap);
//                 value.map(|v| (range, v))
//             })
//             .unzip::<_, _, Vec<_>, Vec<_>>();

//         // Remove these ranges from the string
//         for mut range in ranges {
//             range.start -= offset;
//             range.end -= offset;
//             offset += range.len();
//             s.replace_range(range, "");
//         }

//         values
//     }
//     fn regex_capture_unit(s: &mut String) -> Vec<Self> {
//         let mut offset = 0;
//         let (ranges, values) = A_REGEX
//             .find_iter(&s)
//             .map(|mat| (mat.range(), Self::A))
//             .unzip::<_, _, Vec<_>, Vec<_>>();

//         // Remove these ranges from the string
//         for mut range in ranges {
//             range.start -= offset;
//             range.end -= offset;
//             offset += range.len();
//             s.replace_range(range, "");
//         }

//         values
//     }

//     pub fn from_regex(s: &str) -> Result<Self, ()> {
//         if let Some(variant) = Self::from_regex_as_a_exact(s) {
//             return Ok(variant);
//         }

//         Err(())
//     }

//     // TODO: remove?
//     pub fn regex(&self) -> &Regex {
//         match self {
//             Self::A => &A_REGEX,
//             Self::B => &B_REGEX,
//         }
//     }

//     /// Note: this will allocate a new string from `s`, which will be consumed as items are consumed
//     ///
//     /// TODO: enum variant only
//     /// todo: indices?
//     pub fn search(s: &str) -> Vec<Self> {
//         let mut s = s.to_string();
//         let mut out = Vec::new();
//         out.append(&mut Self::regex_capture_a_consuming(&mut s));

//         out
//     }
// }
// fn test() {
//     let r1 = regex::Regex::new(r"NAS(?P<_0>[0-9][A-z0-9-]*)").unwrap();
//     let r2 = regex::Regex::new(r"AS(?P<_0>[0-9][A-z0-9-]*)").unwrap();

//     let mut text =String::from("Let's talk about NAS1805, NAS1057W4A-028 and S501654321-01. Make sure to also include AS4824N04");

//     let mut offset = 0;
//     for mut range in r1
//         .captures_iter(&text)
//         .map(|cap| {
//             println!("r1: {:?}", cap);
//             cap.get(0).unwrap().range()
//         })
//         .collect::<Vec<_>>()
//     {
//         range.start -= offset;
//         range.end -= offset;
//         offset += range.len();
//         text.replace_range(range, "");
//     }

//     offset = 0;
//     for mut range in r2
//         .captures_iter(&text)
//         .map(|cap| {
//             println!("r2: {:?}", cap);
//             cap.get(0).unwrap().range()
//         })
//         .collect::<Vec<_>>()
//     {
//         range.start -= offset;
//         range.end -= offset;
//         offset += range.len();
//         text.replace_range(range, "");
//     }

//     println!("unmatched: \"{}\"", text)
// }
