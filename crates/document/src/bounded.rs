//! Deserialization limits are checked against declared sequence counts before
//! reserving storage, rather than trusting a network-supplied allocation hint.
use serde::{
    Deserialize, Deserializer,
    de::{Error, SeqAccess, Visitor},
};
use std::{fmt, marker::PhantomData};

pub fn vector<'de, D, T, const LIMIT: usize>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    struct Bounded<T, const N: usize>(PhantomData<T>);
    impl<'de, T: Deserialize<'de>, const N: usize> Visitor<'de> for Bounded<T, N> {
        type Value = Vec<T>;
        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            write!(formatter, "a sequence of at most {N} entries")
        }
        fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<Self::Value, A::Error> {
            if sequence.size_hint().is_some_and(|count| count > N) {
                return Err(A::Error::custom("sequence exceeds protocol limit"));
            }
            let mut values = Vec::with_capacity(sequence.size_hint().unwrap_or(0).min(1_024));
            while let Some(value) = sequence.next_element()? {
                if values.len() == N {
                    return Err(A::Error::custom("sequence exceeds protocol limit"));
                }
                values.push(value);
            }
            Ok(values)
        }
    }
    deserializer.deserialize_seq(Bounded::<T, LIMIT>(PhantomData))
}

pub fn set<'de, D, T, const LIMIT: usize>(
    deserializer: D,
) -> Result<std::collections::BTreeSet<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de> + Ord,
{
    Ok(vector::<D, T, LIMIT>(deserializer)?.into_iter().collect())
}

pub fn map<'de, D, K, V, const LIMIT: usize>(
    deserializer: D,
) -> Result<std::collections::BTreeMap<K, V>, D::Error>
where
    D: Deserializer<'de>,
    K: Deserialize<'de> + Ord,
    V: Deserialize<'de>,
{
    struct Bounded<K, V, const N: usize>(PhantomData<(K, V)>);
    impl<'de, K: Deserialize<'de> + Ord, V: Deserialize<'de>, const N: usize> Visitor<'de>
        for Bounded<K, V, N>
    {
        type Value = std::collections::BTreeMap<K, V>;
        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            write!(formatter, "a map of at most {N} entries")
        }
        fn visit_map<A: serde::de::MapAccess<'de>>(
            self,
            mut entries: A,
        ) -> Result<Self::Value, A::Error> {
            if entries.size_hint().is_some_and(|count| count > N) {
                return Err(A::Error::custom("map exceeds protocol limit"));
            }
            let mut result = std::collections::BTreeMap::new();
            let mut count = 0;
            while let Some((key, value)) = entries.next_entry()? {
                count += 1;
                if count > N {
                    return Err(A::Error::custom("map exceeds protocol limit"));
                }
                if result.insert(key, value).is_some() {
                    return Err(A::Error::custom("duplicate map key"));
                }
            }
            Ok(result)
        }
    }
    deserializer.deserialize_map(Bounded::<K, V, LIMIT>(PhantomData))
}
