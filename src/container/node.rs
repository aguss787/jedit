use std::{fmt::Display, ops::Deref};

use indexmap::IndexMap;
use rayon::iter::{IntoParallelIterator, IntoParallelRefIterator, ParallelIterator};
use serde::Serialize;

use super::INDENT;
use crate::error::{DeserializationError, DumpError, IndexingError, LoadError};

struct Selector<'a, T> {
    keys: &'a [T],
    next_key_pos: usize,
}

impl<'a, T: Deref<Target = str>> Selector<'a, T> {
    fn new(keys: &'a [T]) -> Self {
        Self {
            keys,
            next_key_pos: 0,
        }
    }

    fn next(&mut self) -> Option<&str> {
        let res = self.keys.get(self.next_key_pos);
        self.next_key_pos = (self.next_key_pos + 1).min(self.keys.len());
        res.map(Deref::deref)
    }
}

#[derive(Debug)]
#[cfg_attr(test, derive(PartialEq))]
pub enum Index {
    Terminal,
    Object(Vec<String>),
    Array(usize),
}

#[derive(Debug)]
#[cfg_attr(test, derive(PartialEq))]
pub struct Node {
    n_lines: usize,
    n_bytes: usize,
    data: Kind,
}

#[derive(Debug, Clone, Copy)]
#[cfg_attr(test, derive(PartialEq))]
enum Number {
    Int(i64),
    Float(f64),
}

impl Display for Number {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Number::Int(value) => write!(f, "{}", value),
            Number::Float(value) => write!(f, "{}", value),
        }
    }
}

#[derive(Debug)]
#[cfg_attr(test, derive(PartialEq))]
enum Kind {
    Null,
    Bool(bool),
    Number(Number),
    String(String),
    Array(Vec<Node>),
    Object(IndexMap<String, Node>),
}

impl Node {
    pub fn load(reader: impl std::io::Read) -> Result<Self, LoadError> {
        let value: serde_json::Value = sonic_rs::from_reader(reader)?;
        Self::from_serde_json(value).map_err(Into::into)
    }

    pub fn to_string_pretty(&self) -> Result<String, DumpError> {
        sonic_rs::to_string_pretty(self).map_err(Into::into)
    }

    pub fn subtree<T: Deref<Target = str>>(&self, selector: &[T]) -> Result<&Node, IndexingError> {
        self.subtree_inner(Selector::new(selector))
    }

    pub fn replace<T: Deref<Target = str>>(
        &mut self,
        selector: &[T],
        node: Node,
    ) -> Result<Node, IndexingError> {
        self.replace_inner(Selector::new(selector), node)
    }

    pub fn as_index(&self) -> Index {
        match &self.data {
            Kind::Array(nodes) => Index::Array(nodes.len()),
            Kind::Object(index_map) => Index::Object(index_map.keys().cloned().collect()),
            Kind::Null | Kind::Bool(_) | Kind::Number(_) | Kind::String(_) => Index::Terminal,
        }
    }
}

impl Node {
    pub fn null() -> Self {
        Self {
            n_lines: 1,
            n_bytes: 4,
            data: Kind::Null,
        }
    }

    fn bool(value: bool) -> Self {
        Self {
            n_lines: 1,
            n_bytes: if value { 4 } else { 5 },
            data: Kind::Bool(value),
        }
    }

    fn number(value: serde_json::Number) -> Result<Self, DeserializationError> {
        let n_bytes = serde_json::to_vec(&value).unwrap().len();
        let data = value
            .as_i64()
            .map(Number::Int)
            .or_else(|| value.as_f64().map(Number::Float))
            .ok_or(DeserializationError::InvalidNumber(value))?;
        Ok(Self {
            n_lines: 1,
            n_bytes,
            data: Kind::Number(data),
        })
    }

    fn string(value: String) -> Self {
        Self {
            n_lines: 1,
            n_bytes: value.len() + 2,
            data: Kind::String(value),
        }
    }

    fn array(values: Vec<serde_json::Value>) -> Result<Self, DeserializationError> {
        if values.is_empty() {
            return Ok(Self {
                n_lines: 1,
                n_bytes: 2,
                data: Kind::Array(Vec::new()),
            });
        }

        let nodes: Vec<Self> = values
            .into_par_iter()
            .map(Self::from_serde_json)
            .collect::<Result<_, _>>()?;

        Ok(Self {
            n_lines: nodes.par_iter().map(|node| node.n_lines).sum::<usize>() + 2,
            n_bytes: nodes.par_iter().map(Self::indented_n_bytes).sum::<usize>()
                + nodes.len()
                + nodes.len().saturating_sub(1)
                + 3,
            data: Kind::Array(nodes),
        })
    }

    fn object(values: IndexMap<String, serde_json::Value>) -> Result<Self, DeserializationError> {
        if values.is_empty() {
            return Ok(Self {
                n_lines: 1,
                n_bytes: 2,
                data: Kind::Object(IndexMap::new()),
            });
        }

        let nodes: IndexMap<String, Self> = values
            .into_par_iter()
            .map(|(key, value)| Ok((key, Self::from_serde_json(value)?)))
            .collect::<Result<_, _>>()?;
        Ok(Self {
            n_lines: nodes.par_values().map(|node| node.n_lines).sum::<usize>() + 2,
            n_bytes: nodes
                .par_iter()
                .map(|(key, node)| 4 + key.len() + node.indented_n_bytes())
                .sum::<usize>()
                + nodes.len()
                + nodes.len().saturating_sub(1)
                + 3,
            data: Kind::Object(nodes),
        })
    }

    fn indented_n_bytes(&self) -> usize {
        self.n_bytes + INDENT * self.n_lines
    }

    fn subtree_inner<T: Deref<Target = str>>(
        &self,
        mut selector: Selector<'_, T>,
    ) -> Result<&Self, IndexingError> {
        if let Some(next_key) = selector.next() {
            let missing_key = || IndexingError::MissingKey(next_key.to_string());
            let next_node = match &self.data {
                Kind::Array(nodes) => {
                    let index = next_key.parse::<usize>().map_err(|_| missing_key())?;
                    nodes.get(index).ok_or_else(missing_key)?
                }
                Kind::Object(index_map) => index_map.get(next_key).ok_or_else(missing_key)?,
                Kind::Null | Kind::Bool(_) | Kind::Number(_) | Kind::String(_) => {
                    return Err(IndexingError::NotIndexable);
                }
            };

            next_node.subtree_inner(selector)
        } else {
            Ok(self)
        }
    }

    fn replace_inner<T: Deref<Target = str>>(
        &mut self,
        mut selector: Selector<'_, T>,
        mut new_node: Self,
    ) -> Result<Self, IndexingError> {
        if let Some(next_key) = selector.next() {
            let missing_key = || IndexingError::MissingKey(next_key.to_string());
            let next_node = match &mut self.data {
                Kind::Array(nodes) => {
                    let index = next_key.parse::<usize>().map_err(|_| missing_key())?;
                    nodes.get_mut(index).ok_or_else(missing_key)?
                }
                Kind::Object(index_map) => index_map.get_mut(next_key).ok_or_else(missing_key)?,
                Kind::Null | Kind::Bool(_) | Kind::Number(_) | Kind::String(_) => {
                    return Err(IndexingError::NotIndexable);
                }
            };

            let old_n_lines = next_node.n_lines;
            let old_n_bytes = next_node.indented_n_bytes();
            // TODO recompute meta
            let old_node = next_node.replace_inner(selector, new_node)?;

            self.n_lines = self.n_lines - old_n_lines + next_node.n_lines;
            self.n_bytes = self.n_bytes - old_n_bytes + next_node.indented_n_bytes();

            Ok(old_node)
        } else {
            std::mem::swap(self, &mut new_node);
            Ok(new_node)
        }
    }

    fn from_serde_json(value: serde_json::Value) -> Result<Self, DeserializationError> {
        let res = match value {
            serde_json::Value::Null => Self::null(),
            serde_json::Value::Bool(value) => Self::bool(value),
            serde_json::Value::Number(number) => Self::number(number)?,
            serde_json::Value::String(value) => Self::string(value),
            serde_json::Value::Array(values) => Self::array(values)?,
            serde_json::Value::Object(map) => Self::object(map.into_iter().collect())?,
        };
        Ok(res)
    }
}

impl Serialize for Node {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.data.serialize(serializer)
    }
}

impl Serialize for Kind {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Kind::Null => serde_json::Value::Null.serialize(serializer),
            Kind::Bool(value) => value.serialize(serializer),
            Kind::Number(number) => number.serialize(serializer),
            Kind::String(value) => value.serialize(serializer),
            Kind::Array(nodes) => nodes.serialize(serializer),
            Kind::Object(index_map) => index_map.serialize(serializer),
        }
    }
}

impl Serialize for Number {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Number::Int(value) => value.serialize(serializer),
            Number::Float(value) => value.serialize(serializer),
        }
    }
}

#[cfg(test)]
const RAW_JSON: &str = r#"{
  "string": "something",
  "int": 123,
  "float": 100.3,
  "bool": true,
  "other_bool": false,
  "null": null,
  "array": [
    1,
    2,
    3.0
  ],
  "nested_object": {
    "key": "value"
  }
}"#;

#[cfg(test)]
mod test {
    use super::*;
    use serde_json::json;

    impl Node {
        fn assert_meta(&self) {
            assert_eq!(
                self.to_string_pretty()
                    .unwrap()
                    .lines()
                    .collect::<Vec<_>>()
                    .len(),
                self.n_lines
            );
            assert_eq!(self.to_string_pretty().unwrap().len(), self.n_bytes);
        }

        fn assert_all_meta(&self) {
            self.assert_meta();
            match &self.data {
                Kind::Array(nodes) => nodes.iter().for_each(Self::assert_meta),
                Kind::Object(index_map) => index_map.values().for_each(Self::assert_meta),
                Kind::Null | Kind::Bool(_) | Kind::Number(_) | Kind::String(_) => {}
            }
        }
    }

    #[test]
    fn round_tripe_test() {
        let res = Node::load(RAW_JSON.as_bytes())
            .unwrap()
            .to_string_pretty()
            .unwrap();
        assert_eq!(res, RAW_JSON);
    }

    #[test]
    fn json_value_test() {
        let json_value = json!({
            "string": "something",
            "int": 123,
            "float": 100.3,
            "bool": true,
            "other_bool": false,
            "null": null,
            "array": [1, 2, 3.],
            "nested_object": {
                "key": "value"
            }
        });

        let from_node = Node::from_serde_json(json_value.clone()).unwrap();
        assert_eq!(
            sonic_rs::to_string(&from_node).unwrap(),
            sonic_rs::to_string(&json_value).unwrap(),
        );
    }

    #[test]
    fn node_meta_test() {
        let json_value = json!({
            "string": "something",
            "int": 123,
            "float": 100.3,
            "bool": true,
            "other_bool": false,
            "null": null,
            "array": [
                1,
                2,
                3.
            ],
            "nested_object": {
                "key": "value"
            }
        });

        let node = Node::from_serde_json(json_value.clone()).unwrap();
        node.assert_all_meta();

        let Kind::Object(fields) = node.data else {
            unreachable!()
        };

        assert_eq!(
            fields.keys().collect::<Vec<_>>(),
            [
                "string",
                "int",
                "float",
                "bool",
                "other_bool",
                "null",
                "array",
                "nested_object",
            ]
        );
    }

    #[test]
    fn empty_node_meta_test() {
        for json_value in [
            json!([]),
            json!({}),
            json!(null),
            json!([null]),
            json!(""),
            json!([""]),
        ] {
            let node = Node::from_serde_json(json_value).unwrap();
            node.assert_all_meta();
        }
    }

    #[test]
    fn keys_test() {
        let node = Node::load(RAW_JSON.as_bytes()).unwrap();
        assert_eq!(
            node.subtree::<&str>(&[]).unwrap().as_index(),
            Index::Object(vec![
                String::from("string"),
                String::from("int"),
                String::from("float"),
                String::from("bool"),
                String::from("other_bool"),
                String::from("null"),
                String::from("array"),
                String::from("nested_object"),
            ])
        );

        assert_eq!(
            node.subtree(&["array"]).unwrap().as_index(),
            Index::Array(3)
        );
        assert_eq!(
            node.subtree(&["array", "0"]).unwrap().as_index(),
            Index::Terminal
        );
        assert_eq!(
            node.subtree(&["nested_object"]).unwrap().as_index(),
            Index::Object(vec![String::from("key")])
        );
        assert_eq!(
            node.subtree(&["nested_object", "key"]).unwrap().as_index(),
            Index::Terminal
        );

        assert_eq!(node.subtree(&["int"]).unwrap().as_index(), Index::Terminal);
        assert_eq!(
            node.subtree(&["int", "2"]).unwrap_err(),
            IndexingError::NotIndexable
        );
        assert_eq!(
            node.subtree(&["nested_object", "not_found"]).unwrap_err(),
            IndexingError::MissingKey(String::from("not_found"))
        );
    }

    #[test]
    fn replace_test() {
        let original = json!({
            "a": "x",
            "b": "x",
            "nested": {
                "key": "value"
            },
            "array": [
                1,
                2,
                3
            ]
        });

        let mut node = Node::from_serde_json(original).unwrap();
        let new_node = Node::from_serde_json(json!(["cat", "dog"])).unwrap();
        let replaced_node = node.replace(&["nested", "key"], new_node).unwrap();

        assert_eq!(
            replaced_node,
            Node::from_serde_json(json!("value")).unwrap()
        );
        assert_eq!(
            node,
            Node::from_serde_json(json!({
                "a": "x",
                "b": "x",
                "nested": {
                    "key": [
                        "cat",
                        "dog"
                    ]
                },
                "array": [
                    1,
                    2,
                    3
                ]
            }))
            .unwrap()
        );

        node.assert_all_meta();
    }
}
