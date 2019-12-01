/// A lifetime less DOM implementation. It uses strings to make te
/// structure fully owned, avoiding lifetimes at the cost of performance.
mod cmp;
mod from;
mod serialize;

use crate::value::{MutableValue, Value as ValueTrait, ValueBuilder, ValueType};
use crate::{Deserializer, Node, Result, StaticNode};
use halfbrown::HashMap;
use std::borrow::Borrow;
use std::convert::TryFrom;
use std::fmt;
use std::hash::Hash;
use std::ops::{Index, IndexMut};

/// Representation of a JSON object
pub type Object = HashMap<String, Value>;

/// Parses a slice of bytes into a Value dom. This function will
/// rewrite the slice to de-escape strings.
/// We do not keep any references to the raw data but re-allocate
/// owned memory whereever required thus returning a value without
/// a lifetime.
pub fn to_value(s: &mut [u8]) -> Result<Value> {
    match Deserializer::from_slice(s) {
        Ok(de) => Ok(OwnedDeserializer::from_deserializer(de).parse()),
        Err(e) => Err(e),
    }
}

/// Owned JSON-DOM Value, consider using the `ValueTrait`
/// to access it's content.
/// This is slower then the `BorrowedValue` as a tradeoff
/// for getting rid of lifetimes.
#[derive(Debug, Clone)]
pub enum Value {
    /// Static values
    Static(StaticNode),
    /// string type
    String(String),
    /// array type
    Array(Vec<Value>),
    /// object type
    Object(Box<Object>),
}

impl ValueBuilder for Value {
    #[inline]
    fn null() -> Self {
        Self::Static(StaticNode::Null)
    }
    fn array_with_capacity(capacity: usize) -> Self {
        Self::Array(Vec::with_capacity(capacity))
    }
    fn object_with_capacity(capacity: usize) -> Self {
        Self::Object(Box::new(Object::with_capacity(capacity)))
    }
}

impl MutableValue for Value {
    type Key = String;
    #[inline]
    fn as_array_mut(&mut self) -> Option<&mut Vec<Self>> {
        match self {
            Self::Array(a) => Some(a),
            _ => None,
        }
    }
    #[inline]
    fn as_object_mut(&mut self) -> Option<&mut HashMap<<Self as MutableValue>::Key, Self>> {
        match self {
            Self::Object(m) => Some(m),
            _ => None,
        }
    }
}

impl ValueTrait for Value {
    type Key = String;
    type Array = Vec<Self>;
    type Object = HashMap<Self::Key, Self>;

    #[inline]
    fn get<Q: ?Sized>(&self, k: &Q) -> Option<&Self>
    where
        Self::Key: Borrow<Q> + Hash + Eq,
        Q: Hash + Eq,
    {
        self.as_object().and_then(|a| a.get(k))
    }

    #[inline]
    fn get_idx(&self, i: usize) -> Option<&Self> {
        self.as_array().and_then(|a| a.get(i))
    }

    #[inline]
    fn value_type(&self) -> ValueType {
        match self {
            Self::Static(s) => s.value_type(),
            Self::String(_) => ValueType::String,
            Self::Array(_) => ValueType::Array,
            Self::Object(_) => ValueType::Object,
        }
    }

    #[inline]
    fn is_null(&self) -> bool {
        match self {
            Self::Static(StaticNode::Null) => true,
            _ => false,
        }
    }

    #[inline]
    fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Static(StaticNode::Bool(b)) => Some(*b),
            _ => None,
        }
    }

    #[inline]
    fn as_i64(&self) -> Option<i64> {
        match self {
            Self::Static(StaticNode::I64(i)) => Some(*i),
            Self::Static(StaticNode::U64(i)) => i64::try_from(*i).ok(),
            _ => None,
        }
    }

    #[inline]
    #[allow(clippy::cast_sign_loss)]
    fn as_u64(&self) -> Option<u64> {
        match self {
            Self::Static(StaticNode::I64(i)) => u64::try_from(*i).ok(),
            Self::Static(StaticNode::U64(i)) => Some(*i),
            _ => None,
        }
    }

    #[inline]
    fn as_f64(&self) -> Option<f64> {
        match self {
            Self::Static(StaticNode::F64(i)) => Some(*i),
            _ => None,
        }
    }

    #[inline]
    #[allow(clippy::cast_precision_loss)]
    fn cast_f64(&self) -> Option<f64> {
        match self {
            Self::Static(StaticNode::F64(i)) => Some(*i),
            Self::Static(StaticNode::I64(i)) => Some(*i as f64),
            Self::Static(StaticNode::U64(i)) => Some(*i as f64),
            _ => None,
        }
    }

    #[inline]
    fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(s) => Some(s.as_str()),
            _ => None,
        }
    }

    #[inline]
    fn as_array(&self) -> Option<&Vec<Self>> {
        match self {
            Self::Array(a) => Some(a),
            _ => None,
        }
    }

    #[inline]
    fn as_object(&self) -> Option<&HashMap<Self::Key, Self>> {
        match self {
            Self::Object(m) => Some(m),
            _ => None,
        }
    }
}

#[cfg_attr(tarpaulin, skip)]
impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Static(s) => s.fmt(f),
            Self::String(s) => write!(f, "{}", s),
            Self::Array(a) => write!(f, "{:?}", a),
            Self::Object(o) => write!(f, "{:?}", o),
        }
    }
}

impl Index<&str> for Value {
    type Output = Self;
    fn index(&self, index: &str) -> &Self::Output {
        self.get(index).unwrap()
    }
}

impl Index<usize> for Value {
    type Output = Self;
    fn index(&self, index: usize) -> &Self::Output {
        self.get_idx(index).unwrap()
    }
}

impl IndexMut<&str> for Value {
    fn index_mut(&mut self, index: &str) -> &mut Self::Output {
        self.get_mut(index).unwrap()
    }
}

impl IndexMut<usize> for Value {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        self.get_idx_mut(index).unwrap()
    }
}

impl Default for Value {
    fn default() -> Self {
        Self::Static(StaticNode::Null)
    }
}

struct OwnedDeserializer<'de> {
    de: Deserializer<'de>,
}

impl<'de> OwnedDeserializer<'de> {
    pub fn from_deserializer(de: Deserializer<'de>) -> Self {
        Self { de }
    }
    #[cfg_attr(not(feature = "no-inline"), inline(always))]
    pub fn parse(&mut self) -> Value {
        match self.de.next_() {
            Node::Static(s) => Value::Static(s),
            Node::String(s) => Value::from(s),
            Node::Array(len, _) => self.parse_array(len),
            Node::Object(len, _) => self.parse_map(len),
        }
    }

    #[cfg_attr(not(feature = "no-inline"), inline(always))]
    fn parse_array(&mut self, len: usize) -> Value {
        // Rust doens't optimize the normal loop away here
        // so we write our own avoiding the lenght
        // checks during push
        let mut res = Vec::with_capacity(len);
        unsafe {
            res.set_len(len);
            for i in 0..len {
                std::ptr::write(res.get_unchecked_mut(i), self.parse())
            }
        }
        Value::Array(res)
    }

    #[cfg_attr(not(feature = "no-inline"), inline(always))]
    fn parse_map(&mut self, len: usize) -> Value {
        let mut res = Object::with_capacity(len);

        for _ in 0..len {
            if let Node::String(key) = self.de.next_() {
                // We have to call parse short str twice since parse_short_str
                // does not move the cursor forward
                res.insert_nocheck(key.into(), self.parse());
            } else {
                unreachable!()
            }
        }
        Value::from(res)
    }
}

#[cfg(test)]
mod test {
    #![allow(clippy::cognitive_complexity)]
    use super::*;
    use crate::value::{AccessError, Value as ValueTrait};

    #[test]
    fn object_access() {
        let mut v = Value::null();
        assert_eq!(v.insert("key", ()), Err(AccessError::NotAnObject));
        assert_eq!(v.remove("key"), Err(AccessError::NotAnObject));
        let mut v = Value::object();
        assert_eq!(v.insert("key", 1), Ok(None));
        assert_eq!(v.insert("key", 2), Ok(Some(Value::from(1))));
        assert_eq!(v.remove("key"), Ok(Some(Value::from(2))));
    }

    #[test]
    fn array_access() {
        let mut v = Value::null();
        assert_eq!(v.push("key"), Err(AccessError::NotAnArray));
        assert_eq!(v.pop(), Err(AccessError::NotAnArray));
        let mut v = Value::array();
        assert_eq!(v.push(1), Ok(()));
        assert_eq!(v.push(2), Ok(()));
        assert_eq!(v.pop(), Ok(Some(Value::from(2))));
        assert_eq!(v.pop(), Ok(Some(Value::from(1))));
        assert_eq!(v.pop(), Ok(None));
    }

    #[test]
    fn conversions_i64() {
        let v = Value::from(i64::max_value());
        assert!(v.is_i128());
        assert!(v.is_u128());
        assert!(v.is_i64());
        assert!(v.is_u64());
        assert!(!v.is_i32());
        assert!(!v.is_u32());
        assert!(!v.is_i16());
        assert!(!v.is_u16());
        assert!(!v.is_i8());
        assert!(!v.is_u8());
        assert!(!v.is_f64());
        assert!(!v.is_f32());
        assert!(v.is_f64_castable());
        let v = Value::from(i64::min_value());
        assert!(v.is_i128());
        assert!(!v.is_u128());
        assert!(v.is_i64());
        assert!(!v.is_u64());
        assert!(!v.is_i32());
        assert!(!v.is_u32());
        assert!(!v.is_i16());
        assert!(!v.is_u16());
        assert!(!v.is_i8());
        assert!(!v.is_u8());
        assert!(!v.is_f64());
        assert!(!v.is_f32());
        assert!(v.is_f64_castable());
    }

    #[test]
    fn conversions_i32() {
        let v = Value::from(i32::max_value());
        assert!(v.is_i128());
        assert!(v.is_u128());
        assert!(v.is_i64());
        assert!(v.is_u64());
        assert!(v.is_i32());
        assert!(v.is_u32());
        assert!(!v.is_i16());
        assert!(!v.is_u16());
        assert!(!v.is_i8());
        assert!(!v.is_u8());
        assert!(!v.is_f64());
        assert!(!v.is_f32());
        assert!(v.is_f64_castable());
        let v = Value::from(i32::min_value());
        assert!(v.is_i128());
        assert!(!v.is_u128());
        assert!(v.is_i64());
        assert!(!v.is_u64());
        assert!(v.is_i32());
        assert!(!v.is_u32());
        assert!(!v.is_i16());
        assert!(!v.is_u16());
        assert!(!v.is_i8());
        assert!(!v.is_u8());
        assert!(!v.is_f64());
        assert!(!v.is_f32());
        assert!(v.is_f64_castable());
    }

    #[test]
    fn conversions_i16() {
        let v = Value::from(i16::max_value());
        assert!(v.is_i128());
        assert!(v.is_u128());
        assert!(v.is_i64());
        assert!(v.is_u64());
        assert!(v.is_i32());
        assert!(v.is_u32());
        assert!(v.is_i16());
        assert!(v.is_u16());
        assert!(!v.is_i8());
        assert!(!v.is_u8());
        assert!(!v.is_f64());
        assert!(!v.is_f32());
        assert!(v.is_f64_castable());
        let v = Value::from(i16::min_value());
        assert!(v.is_i128());
        assert!(!v.is_u128());
        assert!(v.is_i64());
        assert!(!v.is_u64());
        assert!(v.is_i32());
        assert!(!v.is_u32());
        assert!(v.is_i16());
        assert!(!v.is_u16());
        assert!(!v.is_i8());
        assert!(!v.is_u8());
        assert!(!v.is_f64());
        assert!(!v.is_f32());
        assert!(v.is_f64_castable());
        assert!(v.is_f64_castable());
    }

    #[test]
    fn conversions_i8() {
        let v = Value::from(i8::max_value());
        assert!(v.is_i128());
        assert!(v.is_u128());
        assert!(v.is_i64());
        assert!(v.is_u64());
        assert!(v.is_i32());
        assert!(v.is_u32());
        assert!(v.is_i16());
        assert!(v.is_u16());
        assert!(v.is_i8());
        assert!(v.is_u8());
        assert!(!v.is_f64());
        assert!(!v.is_f32());
        assert!(v.is_f64_castable());
        let v = Value::from(i8::min_value());
        assert!(v.is_i128());
        assert!(!v.is_u128());
        assert!(v.is_i64());
        assert!(!v.is_u64());
        assert!(v.is_i32());
        assert!(!v.is_u32());
        assert!(v.is_i16());
        assert!(!v.is_u16());
        assert!(v.is_i8());
        assert!(!v.is_u8());
        assert!(!v.is_f64());
        assert!(!v.is_f32());
        assert!(v.is_f64_castable());
    }

    #[test]
    fn conversions_usize() {
        let v = Value::from(usize::min_value() as u64);
        assert!(v.is_i128());
        assert!(v.is_u128());
        assert!(v.is_i64());
        assert!(v.is_u64());
        assert!(v.is_usize());
        assert!(v.is_i32());
        assert!(v.is_u32());
        assert!(v.is_i16());
        assert!(v.is_u16());
        assert!(v.is_i8());
        assert!(v.is_u8());
        assert!(!v.is_f64());
        assert!(!v.is_f32());
        assert!(!v.is_f64());
        assert!(!v.is_f32());
        assert!(v.is_f64_castable());
    }

    #[test]
    fn conversions_u64() {
        let v = Value::from(u64::min_value());
        assert!(v.is_i128());
        assert!(v.is_u128());
        assert!(v.is_i64());
        assert!(v.is_u64());
        assert!(v.is_i32());
        assert!(v.is_u32());
        assert!(v.is_i16());
        assert!(v.is_u16());
        assert!(v.is_i8());
        assert!(v.is_u8());
        assert!(!v.is_f64());
        assert!(!v.is_f32());
        assert!(v.is_f64_castable());
    }

    #[test]
    fn conversions_u32() {
        let v = Value::from(u32::max_value());
        assert!(v.is_i128());
        assert!(v.is_u128());
        assert!(v.is_i64());
        assert!(v.is_u64());
        assert!(!v.is_i32());
        assert!(v.is_u32());
        assert!(!v.is_i16());
        assert!(!v.is_u16());
        assert!(!v.is_i8());
        assert!(!v.is_u8());
        assert!(!v.is_f64());
        assert!(!v.is_f32());
        assert!(v.is_f64_castable());
    }

    #[test]
    fn conversions_u16() {
        let v = Value::from(u16::max_value());
        assert!(v.is_i128());
        assert!(v.is_u128());
        assert!(v.is_i64());
        assert!(v.is_u64());
        assert!(v.is_i32());
        assert!(v.is_u32());
        assert!(!v.is_i16());
        assert!(v.is_u16());
        assert!(!v.is_i8());
        assert!(!v.is_u8());
        assert!(!v.is_f64());
        assert!(!v.is_f32());
        assert!(v.is_f64_castable());
    }

    #[test]
    fn conversions_u8() {
        let v = Value::from(u8::max_value());
        assert!(v.is_i128());
        assert!(v.is_u128());
        assert!(v.is_i64());
        assert!(v.is_u64());
        assert!(v.is_i32());
        assert!(v.is_u32());
        assert!(v.is_i16());
        assert!(v.is_u16());
        assert!(!v.is_i8());
        assert!(v.is_u8());
        assert!(!v.is_f64());
        assert!(!v.is_f32());
        assert!(v.is_f64_castable());
    }

    #[test]
    fn conversions_f64() {
        let v = Value::from(std::f64::MAX);
        assert!(!v.is_i64());
        assert!(!v.is_u64());
        assert!(v.is_f64());
        assert!(!v.is_f32());
        assert!(v.is_f64_castable());
        let v = Value::from(std::f64::MIN);
        assert!(!v.is_i64());
        assert!(!v.is_u64());
        assert!(v.is_f64());
        assert!(!v.is_f32());
        assert!(v.is_f64_castable());
    }

    #[test]
    fn conversions_f32() {
        let v = Value::from(std::f32::MAX);
        assert!(!v.is_i64());
        assert!(!v.is_u64());
        assert!(v.is_f64());
        assert!(v.is_f32());
        assert!(v.is_f64_castable());
        let v = Value::from(std::f32::MIN);
        assert!(!v.is_i64());
        assert!(!v.is_u64());
        assert!(v.is_f64());
        assert!(v.is_f32());
        assert!(v.is_f64_castable());
    }

    #[test]
    fn conversions_array() {
        let v = Value::from(vec![true]);
        assert!(v.is_array());
        assert_eq!(v.value_type(), ValueType::Array);
    }

    #[test]
    fn conversions_bool() {
        let v = Value::from(true);
        assert!(v.is_bool());
        assert_eq!(v.value_type(), ValueType::Bool);
    }

    #[test]
    fn conversions_float() {
        let v = Value::from(42.0);
        assert!(v.is_f64());
        assert_eq!(v.value_type(), ValueType::F64);
    }

    #[test]
    fn conversions_int() {
        let v = Value::from(42);
        assert!(v.is_i64());
        assert_eq!(v.value_type(), ValueType::I64);
    }

    #[test]
    fn conversions_null() {
        let v = Value::from(());
        assert!(v.is_null());
        assert_eq!(v.value_type(), ValueType::Null);
    }

    #[test]
    fn conversions_object() {
        let v = Value::from(Object::new());
        assert!(v.is_object());
        assert_eq!(v.value_type(), ValueType::Object);
    }

    #[test]
    fn conversions_str() {
        let v = Value::from("bla");
        assert!(v.is_str());
        assert_eq!(v.value_type(), ValueType::String);
    }
    use proptest::prelude::*;
    fn arb_value() -> BoxedStrategy<Value> {
        let leaf = prop_oneof![
            Just(Value::Static(StaticNode::Null)),
            any::<bool>()
                .prop_map(StaticNode::Bool)
                .prop_map(Value::Static),
            any::<i64>()
                .prop_map(StaticNode::I64)
                .prop_map(Value::Static),
            any::<f64>()
                .prop_map(StaticNode::F64)
                .prop_map(Value::Static),
            ".*".prop_map(Value::from),
        ];
        leaf.prop_recursive(
            8,   // 8 levels deep
            256, // Shoot for maximum size of 256 nodes
            10,  // We put up to 10 items per collection
            |inner| {
                prop_oneof![
                    // Take the inner strategy and make the two recursive cases.
                    prop::collection::vec(inner.clone(), 0..10).prop_map(Value::Array),
                    prop::collection::hash_map(".*", inner.clone(), 0..10)
                        .prop_map(|m| m.into_iter().collect()),
                ]
            },
        )
        .boxed()
    }

    proptest! {
        #![proptest_config(ProptestConfig {
            .. ProptestConfig::default()
        })]

        #[test]
        fn prop_to_owned(owned in arb_value()) {
            use crate::BorrowedValue;
            let borrowed: BorrowedValue = owned.clone().into();
            prop_assert_eq!(owned, borrowed);
        }

        #[test]
        fn prop_serialize_deserialize(owned in arb_value()) {
            let mut string = owned.encode();
            let mut bytes = unsafe{ string.as_bytes_mut()};
            let decoded = to_value(&mut bytes).expect("Failed to decode");
            prop_assert_eq!(owned, decoded)
        }
        #[test]
        #[allow(clippy::float_cmp)]
        fn prop_f64_cmp(f in proptest::num::f64::NORMAL) {
            let v: Value = f.into();
            prop_assert_eq!(v, f)

        }

        #[test]
        #[allow(clippy::float_cmp)]
        fn prop_f32_cmp(f in proptest::num::f32::NORMAL) {
            let v: Value = f.into();
            prop_assert_eq!(v, f)

        }
        #[test]
        fn prop_i64_cmp(f in proptest::num::i64::ANY) {
            let v: Value = f.into();
            prop_assert_eq!(v, f)
        }
        #[test]
        fn prop_i32_cmp(f in proptest::num::i32::ANY) {
            let v: Value = f.into();
            prop_assert_eq!(v, f)
        }
        #[test]
        fn prop_i16_cmp(f in proptest::num::i16::ANY) {
            let v: Value = f.into();
            prop_assert_eq!(v, f)
        }
        #[test]
        fn prop_i8_cmp(f in proptest::num::i8::ANY) {
            let v: Value = f.into();
            prop_assert_eq!(v, f)
        }
        #[test]
        fn prop_u64_cmp(f in proptest::num::u64::ANY) {
            let v: Value = f.into();
            prop_assert_eq!(v, f)
        }

        #[test]
        #[allow(clippy::cast_possible_truncation)]
        fn prop_usize_cmp(f in proptest::num::usize::ANY) {
            let v: Value = f.into();
            prop_assert_eq!(v, f)
        }
         #[test]
        fn prop_u32_cmp(f in proptest::num::u32::ANY) {
            let v: Value = f.into();
            prop_assert_eq!(v, f)
        }
        #[test]
        fn prop_u16_cmp(f in proptest::num::u16::ANY) {
            let v: Value = f.into();
            prop_assert_eq!(v, f)
        }
        #[test]
        fn prop_u8_cmp(f in proptest::num::u8::ANY) {
            let v: Value = f.into();
            prop_assert_eq!(v.clone(), &f);
            prop_assert_eq!(v, f);
        }
        #[test]
        fn prop_string_cmp(f in ".*") {
            let v: Value = f.clone().into();
            prop_assert_eq!(v.clone(), f.as_str());
            prop_assert_eq!(v, f);
        }

    }
    #[test]
    fn test_union_cmp() {
        let v: Value = ().into();
        assert_eq!(v, ())
    }
    #[test]
    fn test_bool_cmp() {
        let v: Value = true.into();
        assert_eq!(v, true);
        let v: Value = false.into();
        assert_eq!(v, false);
    }
}
