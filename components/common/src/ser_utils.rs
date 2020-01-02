use std::collections::HashMap;
use std::convert::TryFrom;
use std::fmt;
use std::hash::Hash;
use std::marker::PhantomData;
use std::str::FromStr;
use std::string::ToString;

use serde::de::{Deserialize, Error, MapAccess, Visitor};
use serde::ser::{Serialize, SerializeMap, Serializer};
use serde::Deserializer;

use base64::{self, URL_SAFE_NO_PAD};

pub trait SerBase64<'de>: Sized {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer;
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>;
}

impl<'de, T> SerBase64<'de> for T
where
    T: AsRef<[u8]> + for<'t> TryFrom<&'t [u8]>,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let base64_str = base64::encode_config(&self.as_ref(), URL_SAFE_NO_PAD);
        serializer.serialize_str(&base64_str)
    }

    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct ItemVisitor<T> {
            item: PhantomData<T>,
        }

        impl<'de, T> Visitor<'de> for ItemVisitor<T>
        where
            T: for<'t> TryFrom<&'t [u8]>,
        {
            type Value = T;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("A bytes like item")
            }

            fn visit_str<E>(self, str_item: &str) -> Result<Self::Value, E>
            where
                E: Error,
            {
                let vec = base64::decode_config(&str_item, URL_SAFE_NO_PAD)
                    .map_err(|err| Error::custom(err.to_string()))?;
                T::try_from(&vec).map_err(|_| Error::custom("Length mismatch"))
            }
        }

        let visitor = ItemVisitor { item: PhantomData };
        deserializer.deserialize_str(visitor)
    }
}

pub trait SerString<'de>: Sized {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer;
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>;
}

impl<'de, T> SerString<'de> for T
where
    T: ToString + FromStr,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }

    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct ItemVisitor<T> {
            item: PhantomData<T>,
        }

        impl<'de, T> Visitor<'de> for ItemVisitor<T>
        where
            T: FromStr,
        {
            type Value = T;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("A bytes like item")
            }

            fn visit_str<E>(self, str_item: &str) -> Result<Self::Value, E>
            where
                E: Error,
            {
                str_item
                    .parse()
                    .map_err(|_| Error::custom("Failed to parse as string"))
            }
        }

        let visitor = ItemVisitor { item: PhantomData };
        deserializer.deserialize_str(visitor)
    }
}

/*
/// Serializes `buffer` to a lowercase hex string.
pub fn to_base64<T, S>(to_base64: &T, serializer: S) -> Result<S::Ok, S::Error>
where
    T: AsRef<[u8]>,
    S: Serializer,
{
    let base64_str = base64::encode_config(&to_base64.as_ref(), URL_SAFE_NO_PAD);
    serializer.serialize_str(&base64_str)
}

/// Deserializes a lowercase hex string to a `Vec<u8>`.
pub fn from_base64<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    T: for<'t> TryFrom<&'t [u8]>,
    D: Deserializer<'de>,
{
    let string = String::deserialize(deserializer)?;
    let vec = base64::decode_config(&string, URL_SAFE_NO_PAD)
        .map_err(|err| Error::custom(err.to_string()))?;
    T::try_from(&vec).map_err(|_| Error::custom("Length mismatch"))
}

/// Serializes value as a string
pub fn to_string<T, S>(input: &T, serializer: S) -> Result<S::Ok, S::Error>
where
    T: ToString,
    S: Serializer,
{
    serializer.serialize_str(&input.to_string())
}

/// Deserializes a string into a value
pub fn from_string<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    T: FromStr,
    D: Deserializer<'de>,
{
    String::deserialize(deserializer)?
        .parse()
        .map_err(|_| Error::custom("Failed to parse as string"))
}
*/

/// A util for serializing HashMaps with keys that are not strings.
/// For example: JSON serialization does not allow keys that are not strings.
/// SerHashMap first converts the key to a base64 string, and only then serializes.
pub trait SerMapB64Any<'de>: Sized {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer;
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>;
}

impl<'de, K, V> SerMapB64Any<'de> for HashMap<K, V>
where
    K: Serialize + Deserialize<'de> + AsRef<[u8]> + for<'t> TryFrom<&'t [u8]> + Eq + Hash,
    V: Serialize + Deserialize<'de>,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = serializer.serialize_map(Some(self.len()))?;
        for (k, v) in self {
            let string_k = base64::encode_config(k.as_ref(), URL_SAFE_NO_PAD);
            map.serialize_entry(&string_k, v)?;
        }
        map.end()
    }

    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct MapVisitor<K, V> {
            key: PhantomData<K>,
            value: PhantomData<V>,
        }

        impl<'de, K, V> Visitor<'de> for MapVisitor<K, V>
        where
            K: Deserialize<'de> + for<'t> TryFrom<&'t [u8]> + Eq + Hash,
            V: Deserialize<'de>,
        {
            type Value = HashMap<K, V>;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("A map")
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                let mut res_map = HashMap::new();
                while let Some((k_string, v)) = map.next_entry::<String, V>()? {
                    let vec = base64::decode_config(&k_string, URL_SAFE_NO_PAD)
                        .map_err(|err| Error::custom(err.to_string()))?;
                    let k = K::try_from(&vec).map_err(|_| Error::custom("Length mismatch"))?;

                    res_map.insert(k, v);
                }
                Ok(res_map)
            }
        }

        let visitor = MapVisitor {
            key: PhantomData,
            value: PhantomData,
        };
        deserializer.deserialize_map(visitor)
    }
}

// ===============================================================

pub trait SerMapStrAny<'de>: Sized {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer;
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>;
}

impl<'de, K, V> SerMapStrAny<'de> for HashMap<K, V>
where
    K: Serialize + Deserialize<'de> + ToString + FromStr + Eq + Hash,
    V: Serialize + Deserialize<'de>,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = serializer.serialize_map(Some(self.len()))?;
        for (k, v) in self {
            map.serialize_entry(&k.to_string(), v)?;
        }
        map.end()
    }

    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct MapVisitor<K, V> {
            key: PhantomData<K>,
            value: PhantomData<V>,
        }

        impl<'de, K, V> Visitor<'de> for MapVisitor<K, V>
        where
            K: Deserialize<'de> + FromStr + Eq + Hash,
            V: Deserialize<'de>,
        {
            type Value = HashMap<K, V>;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("A map")
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                let mut res_map = HashMap::new();
                while let Some((k_string, v)) = map.next_entry::<String, V>()? {
                    let k = k_string
                        .parse()
                        .map_err(|_| Error::custom("Parse failed"))?;

                    res_map.insert(k, v);
                }
                Ok(res_map)
            }
        }

        let visitor = MapVisitor {
            key: PhantomData,
            value: PhantomData,
        };
        deserializer.deserialize_map(visitor)
    }
}

// ===============================================================

pub trait SerMapStrStr<'de>: Sized {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer;
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>;
}

impl<'de, K, V> SerMapStrStr<'de> for HashMap<K, V>
where
    K: Serialize + Deserialize<'de> + FromStr + ToString + Eq + Hash,
    V: Serialize + Deserialize<'de> + FromStr + ToString,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = serializer.serialize_map(Some(self.len()))?;
        for (k, v) in self {
            map.serialize_entry(&k.to_string(), &v.to_string())?;
        }
        map.end()
    }

    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct MapVisitor<K, V> {
            key: PhantomData<K>,
            value: PhantomData<V>,
        }

        impl<'de, K, V> Visitor<'de> for MapVisitor<K, V>
        where
            K: Serialize + Deserialize<'de> + FromStr + Eq + Hash,
            V: Serialize + Deserialize<'de> + FromStr,
        {
            type Value = HashMap<K, V>;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("A map")
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                let mut res_map = HashMap::new();
                while let Some((k_string, v_string)) = map.next_entry::<String, String>()? {
                    let k = k_string
                        .parse()
                        .map_err(|_| Error::custom("Parse failed"))?;

                    let v = v_string
                        .parse()
                        .map_err(|_| Error::custom("Parse failed"))?;

                    res_map.insert(k, v);
                }
                Ok(res_map)
            }
        }

        let visitor = MapVisitor {
            key: PhantomData,
            value: PhantomData,
        };
        deserializer.deserialize_map(visitor)
    }
}

// =========================================================================

pub trait SerOptionB64<'de>: Sized {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer;
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>;
}

impl<'de, T> SerOptionB64<'de> for Option<T>
where
    T: Serialize + Deserialize<'de> + AsRef<[u8]> + for<'t> TryFrom<&'t [u8]>,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Some(item) => {
                let string_item = base64::encode_config(item.as_ref(), URL_SAFE_NO_PAD);
                serializer.serialize_some(&string_item)
            }
            None => serializer.serialize_none(),
        }
    }

    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct ItemVisitor<T> {
            item: PhantomData<T>,
        }

        impl<'de, T> Visitor<'de> for ItemVisitor<T>
        where
            T: Deserialize<'de> + for<'t> TryFrom<&'t [u8]>,
        {
            type Value = Option<T>;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("An option")
            }

            fn visit_none<E>(self) -> Result<Self::Value, E>
            where
                E: Error,
            {
                Ok(None)
            }

            fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
            where
                D: Deserializer<'de>,
            {
                struct B64Visitor<T> {
                    item: PhantomData<T>,
                }

                impl<'de, T> Visitor<'de> for B64Visitor<T>
                where
                    T: for<'t> TryFrom<&'t [u8]>,
                {
                    type Value = T;

                    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                        formatter.write_str("A bytes like item")
                    }

                    fn visit_str<E>(self, str_item: &str) -> Result<Self::Value, E>
                    where
                        E: Error,
                    {
                        let vec = base64::decode_config(&str_item, URL_SAFE_NO_PAD)
                            .map_err(|err| Error::custom(err.to_string()))?;
                        T::try_from(&vec).map_err(|_| Error::custom("Length mismatch"))
                    }
                }

                let b64_visitor = B64Visitor { item: PhantomData };
                Ok(Some(deserializer.deserialize_string(b64_visitor)?))
            }
        }

        let visitor = ItemVisitor { item: PhantomData };
        deserializer.deserialize_option(visitor)
    }
}
