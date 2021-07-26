extern crate from_regex_macros;
pub use from_regex_macros::*;
pub use lazy_static::lazy_static;

pub use regex::{self, Captures, Regex};
pub use segmap::{self, SegmentMap};
pub use std::str::FromStr;

// TODO: String vs &str in capture fields
// TODO: only need clone for search. And not really even for that

/// Try to construct an instance of this type from a string
pub trait FromRegex: Sized {
    /// Try to construct an instance of this type from a string
    fn from_regex(s: &str) -> Option<Self>;

    /// Search through a string and return all instances of this type matched
    fn matches(s: &str) -> Vec<Self> {
        Self::match_locations(s)
            .into_iter()
            .map(|(_, v)| v)
            .collect()
    }

    /// Search through a string and return all instances of this type matched,
    /// As well as the ranges at which they occur.
    fn match_locations(s: &str) -> SegmentMap<usize, Self>;
}

// TODO: Search trait? to split matches/match_locations out...

// #[cfg(feature = "from_str")]
// impl<T: FromRegex> std::str::FromStr for T {
//     type Err = FromRegexError<T::CustomError>;
//     fn from_str(s: &str) -> Result<Self, Self::Err> {
//         T::from_regex(s)
//     }
// }

pub trait TextMap<V> {
    fn merge_only_longest<I: IntoIterator<Item = (segmap::Segment<usize>, V)>>(&mut self, other: I);
}

impl<V: Clone + Eq> TextMap<V> for SegmentMap<usize, V> {
    fn merge_only_longest<I: IntoIterator<Item = (segmap::Segment<usize>, V)>>(
        &mut self,
        other: I,
    ) {
        for (range, value) in other {
            // Check the easy case (insert into an empty space)
            if let Some(value) = self.insert_if_empty(range, value) {
                // Check using start and end (if we're keeping whichever is longer,
                // one of them must be overlapped by a longer segment)
                let to_remove =
                    if let (Some(start), Some(end)) = (range.start_value(), range.end_value()) {
                        let len = end - start;

                        // Check if this range is larger than the overlapping range preceeding it
                        let before = self.get_range_value(start).map(|(r, _)| r);
                        let larger_than_before = if let Some(before) = before {
                            before
                                .start_value()
                                .zip(before.end_value())
                                .map(|(a, b)| len > (b - a))
                                .unwrap_or_default()
                        } else {
                            // If no range before, treat this as larger (i.e. insert it)
                            true
                        };

                        // Likewise for the range after
                        let after = self.get_range_value(end).map(|(r, _)| r);
                        let larger_than_after = if let Some(after) = after {
                            after
                                .start_value()
                                .zip(after.end_value())
                                .map(|(a, b)| len > (b - a))
                                .unwrap_or_default()
                        } else {
                            true
                        };

                        // Return cloned ranges so they aren't borrowed
                        if larger_than_before && larger_than_after {
                            Some((before.cloned(), after.cloned()))
                        } else {
                            None
                        }
                    } else {
                        None
                    };

                if let Some((before, after)) = to_remove {
                    if let Some(before) = before {
                        self.clear_range(before);
                    }
                    if let Some(after) = after {
                        self.clear_range(after);
                    }

                    // Add the current range (and overwrite any ranges it fully encompasses)
                    self.set(range, value);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
