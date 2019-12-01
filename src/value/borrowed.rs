///A dom object that references the raw input data to avoid allocations
// it tradecs having lifetimes for a gain in performance.
mod cmp;
mod from;
mod serialize;

use crate::value::{MutableValue, Value as ValueTrait, ValueBuilder, ValueType};
use crate::{Deserializer, Node, Result, StaticNode};
use halfbrown::HashMap;
use std::borrow::{Borrow, Cow};
use std::convert::TryFrom;
use std::fmt;
use std::hash::Hash;
use std::ops::{Index, IndexMut};

/// Representation of a JSON object
pub type Object<'v> = HashMap<Cow<'v, str>, Value<'v>>;

/// Parses a slice of butes into a Value dom. This function will
/// rewrite the slice to de-escape strings.
/// As we reference parts of the input slice the resulting dom
/// has the dame lifetime as the slice it was created from.
pub fn to_value<'v>(s: &'v mut [u8]) -> Result<Value<'v>> {
    match Deserializer::from_slice(s) {
        Ok(de) => Ok(BorrowDeserializer::from_deserializer(de).parse()),
        Err(e) => Err(e),
    }
}

/// Borrowed JSON-DOM Value, consider using the `ValueTrait`
/// to access its content
#[derive(Debug, Clone)]
pub enum Value<'v> {
    /// Static values
    Static(StaticNode),
    /// string type
    String(Cow<'v, str>),
    /// array type
    Array(Vec<Value<'v>>),
    /// object type
    Object(Box<Object<'v>>),
}

impl<'v> Value<'v> {
    /// Enforces static lifetime on a borrowed value, this will
    /// force all strings to become owned COW's, the same applies for
    /// Object keys.
    pub fn into_static(self) -> Value<'static> {
        unsafe {
            use std::mem::transmute;
            transmute(match self {
                Self::String(Cow::Borrowed(s)) => Self::String(Cow::Owned(s.to_owned())),
                Self::Array(arr) => arr.into_iter().map(Value::into_static).collect(),
                Self::Object(obj) => obj
                    .into_iter()
                    .map(|(k, v)| (Cow::Owned(k.into_owned()), v.into_static()))
                    .collect(),
                _ => self,
            })
        }
    }

    /// Clones the current value and enforces a static lifetime, it works the same
    /// as `into_static` but includes cloning logic
    pub fn clone_static(&self) -> Value<'static> {
        unsafe {
            use std::mem::transmute;
            transmute(match self {
                Self::String(s) => Self::String(Cow::Owned(s.to_string())),
                Self::Array(arr) => arr.iter().map(Value::clone_static).collect(),
                Self::Object(obj) => obj
                    .iter()
                    .map(|(k, v)| (Cow::Owned(k.to_string()), v.clone_static()))
                    .collect(),
                Self::Static(s) => Self::Static(*s),
            })
        }
    }
}

impl<'v> ValueBuilder for Value<'v> {
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

impl<'v> MutableValue for Value<'v> {
    type Key = Cow<'v, str>;
    #[inline]
    fn as_array_mut(&mut self) -> Option<&mut Vec<Value<'v>>> {
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

impl<'v> ValueTrait for Value<'v> {
    type Key = Cow<'v, str>;
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
            Self::String(s) => Some(s.borrow()),
            _ => None,
        }
    }

    #[inline]
    fn as_array(&self) -> Option<&Vec<Value<'v>>> {
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
impl<'v> fmt::Display for Value<'v> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Static(s) => write!(f, "{}", s),
            Self::String(s) => write!(f, "{}", s),
            Self::Array(a) => write!(f, "{:?}", a),
            Self::Object(o) => write!(f, "{:?}", o),
        }
    }
}

impl<'v> Index<&str> for Value<'v> {
    type Output = Value<'v>;
    fn index(&self, index: &str) -> &Self::Output {
        self.get(index).unwrap()
    }
}

impl<'v> Index<usize> for Value<'v> {
    type Output = Value<'v>;
    fn index(&self, index: usize) -> &Self::Output {
        self.get_idx(index).unwrap()
    }
}

impl<'v> IndexMut<&str> for Value<'v> {
    fn index_mut(&mut self, index: &str) -> &mut Self::Output {
        self.get_mut(index).unwrap()
    }
}

impl<'v> IndexMut<usize> for Value<'v> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        self.get_idx_mut(index).unwrap()
    }
}

impl<'v> Default for Value<'v> {
    fn default() -> Self {
        Self::Static(StaticNode::Null)
    }
}

struct BorrowDeserializer<'de>(Deserializer<'de>);

impl<'de> BorrowDeserializer<'de> {
    pub fn from_deserializer(de: Deserializer<'de>) -> Self {
        Self(de)
    }

    #[cfg_attr(not(feature = "no-inline"), inline(always))]
    pub fn parse(&mut self) -> Value<'de> {
        match self.0.next_() {
            Node::Static(s) => Value::Static(s),
            Node::String(s) => Value::from(s),
            Node::Array(len, _) => self.parse_array(len),
            Node::Object(len, _) => self.parse_map(len),
        }
    }

    #[cfg_attr(not(feature = "no-inline"), inline(always))]
    fn parse_array(&mut self, len: usize) -> Value<'de> {
        // Rust doens't optimize the normal loop away here
        // so we write our own avoiding the lenght
        // checks during push
        let mut res = Vec::with_capacity(len);
        unsafe {
            res.set_len(len);
            for i in 0..len {
                std::ptr::write(res.get_unchecked_mut(i), self.parse());
            }
        }
        Value::Array(res)
    }

    #[cfg_attr(not(feature = "no-inline"), inline(always))]
    fn parse_map(&mut self, len: usize) -> Value<'de> {
        let mut res = Object::with_capacity(len);

        // Since we checked if it's empty we know that we at least have one
        // element so we eat this
        for _ in 0..len {
            if let Node::String(key) = self.0.next_() {
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
    fn arb_value() -> BoxedStrategy<Value<'static>> {
        let leaf = prop_oneof![
            Just(Value::Static(StaticNode::Null)),
            any::<bool>()
                .prop_map(StaticNode::Bool)
                .prop_map(Value::Static),
            any::<i64>()
                .prop_map(StaticNode::I64)
                .prop_map(Value::Static),
            any::<u64>()
                .prop_map(StaticNode::U64)
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
                    prop::collection::hash_map(".*".prop_map(Cow::Owned), inner, 0..10)
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
        fn prop_to_owned(borrowed in arb_value()) {
            use crate::OwnedValue;
            let owned: OwnedValue = borrowed.clone().into();
            prop_assert_eq!(borrowed, owned);
        }
        #[test]
        fn prop_into_static(borrowed in arb_value()) {
            let static_borrowed = borrowed.clone().into_static();
            assert_eq!(borrowed, static_borrowed);
        }
        #[test]
        fn prop_clone_static(borrowed in arb_value()) {
            let static_borrowed = borrowed.clone_static();
            assert_eq!(borrowed, static_borrowed);
        }
        #[test]
        fn prop_serialize_deserialize(borrowed in arb_value()) {
            let mut string = borrowed.encode();
            let mut bytes = unsafe{ string.as_bytes_mut()};
            let decoded = to_value(&mut bytes).expect("Failed to decode");
            prop_assert_eq!(borrowed, decoded)
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
            assert_eq!(v, &f);
            prop_assert_eq!(v, f)
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
