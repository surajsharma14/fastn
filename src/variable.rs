#[derive(Debug, PartialEq, Clone, serde::Serialize, serde::Deserialize)]
pub struct Variable {
    pub name: String,
    pub value: ftd::Value,
    pub conditions: Vec<(ftd::p2::Boolean, ftd::Value)>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
pub enum PropertyValue {
    Value { value: ftd::variable::Value },
    Reference { name: String, kind: ftd::p2::Kind },
    Variable { name: String, kind: ftd::p2::Kind },
}

impl PropertyValue {
    pub fn resolve_value(
        line_number: usize,
        value: &str,
        expected_kind: Option<ftd::p2::Kind>,
        doc: &ftd::p2::TDoc,
        arguments: &std::collections::BTreeMap<String, ftd::p2::Kind>,
        source: Option<ftd::TextSource>,
    ) -> ftd::p1::Result<ftd::PropertyValue> {
        let property_type = if let Some(arg) = value.strip_prefix('$') {
            PropertyType::Variable(arg.to_string())
        } else if let Some(ftd::p2::Kind::UI { .. }) = expected_kind {
            if !value.contains(':') {
                return ftd::e2(
                    format!("expected `:`, found: `{}`", value),
                    doc.name,
                    line_number,
                );
            }
            let (name, caption) = ftd::p2::utils::split(value.to_string(), ":")?;
            PropertyType::Component { name, caption }
        } else {
            let value = if let Some(value) = value.strip_prefix('\\') {
                value.to_string()
            } else {
                value.to_string()
            };
            PropertyType::Value(value)
        };

        let (part1, part2) = ftd::p2::utils::get_doc_name_and_remaining(&property_type.string())?;

        return Ok(match property_type {
            PropertyType::Variable(string) | PropertyType::Component { name: string, .. } => {
                let (kind, is_doc) = match arguments.get(&part1) {
                    _ if part1.eq("MOUSE-IN") => (
                        ftd::p2::Kind::Boolean {
                            default: Some("false".to_string()),
                        },
                        false,
                    ),
                    None => match doc.get_thing(line_number, &string) {
                        Ok(ftd::p2::Thing::Variable(v)) => (v.value.kind(), true),
                        Ok(ftd::p2::Thing::Component(_)) => {
                            (ftd::p2::Kind::UI { default: None }, true)
                        }
                        e => {
                            return ftd::e2(
                                format!("{} is not present in doc, {:?}", part1, e),
                                doc.name,
                                line_number,
                            )
                        }
                    },
                    Some(kind) => (kind.to_owned(), false),
                };

                let found_kind = get_kind(line_number, &kind, part2, doc, &expected_kind)?;
                if is_doc {
                    PropertyValue::Reference {
                        name: doc
                            .resolve_name(line_number, string.as_str())
                            .unwrap_or(string),
                        kind: found_kind,
                    }
                } else {
                    PropertyValue::Variable {
                        name: string,
                        kind: found_kind,
                    }
                }
            }
            PropertyType::Value(string) => {
                if expected_kind.is_none() {
                    return ftd::e2(
                        "expected expected_kind while calling resolve_value",
                        doc.name,
                        line_number,
                    );
                }
                let expected_kind = expected_kind.unwrap();
                match expected_kind.inner() {
                    ftd::p2::Kind::Integer { .. } => ftd::PropertyValue::Value {
                        value: ftd::Value::Integer {
                            value: string.parse::<i64>().map_err(|e| {
                                ftd::p1::Error::ParseError {
                                    message: e.to_string(),
                                    doc_id: doc.name.to_string(),
                                    line_number,
                                }
                            })?,
                        },
                    },
                    ftd::p2::Kind::Decimal { .. } => ftd::PropertyValue::Value {
                        value: ftd::Value::Decimal {
                            value: string.parse::<f64>().map_err(|e| {
                                ftd::p1::Error::ParseError {
                                    message: e.to_string(),
                                    doc_id: doc.name.to_string(),
                                    line_number,
                                }
                            })?,
                        },
                    },
                    ftd::p2::Kind::Boolean { .. } => ftd::PropertyValue::Value {
                        value: ftd::Value::Boolean {
                            value: string.parse::<bool>().map_err(|e| {
                                ftd::p1::Error::ParseError {
                                    message: e.to_string(),
                                    doc_id: doc.name.to_string(),
                                    line_number,
                                }
                            })?,
                        },
                    },
                    ftd::p2::Kind::String { .. } => ftd::PropertyValue::Value {
                        value: ftd::Value::String {
                            text: string,
                            source: source.unwrap_or(ftd::TextSource::Header),
                        },
                    },
                    t => {
                        return ftd::e2(
                            format!("can't resolve value {} to expected kind {:?}", string, t),
                            doc.name,
                            line_number,
                        )
                    }
                }
            }
        });

        #[derive(Debug)]
        enum PropertyType {
            Value(String),
            Variable(String),
            Component { name: String, caption: String },
        }

        impl PropertyType {
            fn string(&self) -> String {
                match self {
                    PropertyType::Value(s)
                    | PropertyType::Variable(s)
                    | PropertyType::Component { name: s, .. } => s.to_string(),
                }
            }
        }

        fn get_kind(
            line_number: usize,
            kind: &ftd::p2::Kind,
            p2: Option<String>,
            doc: &ftd::p2::TDoc,
            expected_kind: &Option<ftd::p2::Kind>,
        ) -> ftd::p1::Result<ftd::p2::Kind> {
            let mut found_kind = kind.to_owned();
            if let ftd::p2::Kind::Record { ref name, .. } = kind {
                if let Some(p2) = p2 {
                    let rec = doc.get_record(line_number, &doc.resolve_name(line_number, name)?)?;
                    found_kind = match rec.fields.get(p2.as_str()) {
                        Some(kind) => kind.to_owned(),
                        _ => {
                            return ftd::e2(
                                format!("{} is not present in {} of type {:?}", p2, name, rec),
                                doc.name,
                                line_number,
                            );
                        }
                    };
                }
            }
            if let Some(e_kind) = expected_kind {
                if !e_kind.is_same_as(&found_kind) {
                    return ftd::e2(
                        format!("expected {:?} found {:?}", found_kind, e_kind),
                        doc.name,
                        line_number,
                    );
                }
                return Ok(e_kind.to_owned().set_default({
                    if found_kind.get_default_value_str().is_some() {
                        found_kind.get_default_value_str()
                    } else {
                        e_kind.get_default_value_str()
                    }
                }));
            }
            Ok(found_kind)
        }
    }

    pub fn kind(&self) -> ftd::p2::Kind {
        match self {
            Self::Value { value: v } => v.kind(),
            Self::Reference { kind, .. } => kind.to_owned(),
            Self::Variable { kind, .. } => kind.to_owned(),
        }
    }
    pub fn resolve(
        &self,
        line_number: usize,
        arguments: &std::collections::BTreeMap<String, ftd::Value>,
        doc: &ftd::p2::TDoc,
    ) -> ftd::p1::Result<Value> {
        let mut value = match self {
            ftd::PropertyValue::Value { value: v } => v.to_owned(),
            ftd::PropertyValue::Variable {
                name,
                kind: argument_kind,
            } => {
                assert_eq!(self.kind(), *argument_kind);
                if name.contains('.') {
                    let (part_1, part_2) = ftd::p2::utils::split(name.to_string(), ".")?;
                    match arguments.get(&part_1) {
                        Some(Value::Record { name, fields }) => match fields.get(&part_2) {
                            Some(pv) => return pv.resolve(line_number, arguments, doc),
                            None => {
                                return ftd::e2(
                                    format!(
                                        "{} is not present in record {} [name: {}]",
                                        part_2, part_1, name
                                    ),
                                    doc.name,
                                    line_number,
                                )
                            }
                        },
                        None => {
                            return ftd::e2(
                                format!("{} is not present in argument [name: {}]", part_1, name),
                                doc.name,
                                line_number,
                            );
                        }
                        _ => {
                            return ftd::e2(
                                format!("{} is not a record [name: {}]", part_1, name),
                                doc.name,
                                line_number,
                            )
                        }
                    }
                } else {
                    match (arguments.get(name.as_str()), argument_kind.is_optional()) {
                        (Some(v), _) => v.to_owned(),
                        (None, t) => {
                            if let Ok(val) = argument_kind.to_value(line_number, doc.name) {
                                val
                            } else {
                                if !t {
                                    return ftd::e2(
                                        format!("{} is required", name),
                                        doc.name,
                                        line_number,
                                    );
                                }
                                Value::None {
                                    kind: argument_kind.to_owned(),
                                }
                            }
                        }
                    }
                }
            }
            ftd::PropertyValue::Reference {
                name: reference_name,
                kind: reference_kind,
            } => {
                assert_eq!(self.kind(), *reference_kind);
                let (default, condition) =
                    if let Ok(d) = doc.get_value_and_conditions(0, reference_name.as_str()) {
                        d
                    } else if let Ok(d) = doc.get_component(0, reference_name.as_str()) {
                        return d.to_value(reference_kind);
                    } else {
                        return reference_kind.to_value(line_number, doc.name);
                    };
                let mut value = default;
                for (boolean, property) in condition {
                    if boolean.eval(line_number, arguments, doc)? {
                        value = property;
                    }
                }
                value
            }
        };

        // In case of Object resolve all the values
        if let ftd::Value::Object { values } = &mut value {
            for (_, v) in values.iter_mut() {
                *v = ftd::PropertyValue::Value {
                    value: v.resolve(line_number, arguments, doc)?,
                };
            }
        }
        Ok(value)
    }
}

#[derive(Debug, PartialEq, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
pub enum TextSource {
    Header,
    Caption,
    Body,
    Default,
}

impl TextSource {
    pub fn from_kind(
        kind: &ftd::p2::Kind,
        doc_id: &str,
        line_number: usize,
    ) -> ftd::p1::Result<Self> {
        Ok(match kind {
            ftd::p2::Kind::String { caption, body, .. } => {
                if *caption {
                    TextSource::Caption
                } else if *body {
                    TextSource::Body
                } else {
                    TextSource::Header
                }
            }
            t => {
                return ftd::e2(
                    format!("expected string kind, found: {:?}", t),
                    doc_id,
                    line_number,
                )
            }
        })
    }
}

#[derive(Debug, PartialEq, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
#[allow(clippy::large_enum_variant)]
pub enum Value {
    None {
        kind: ftd::p2::Kind,
    },
    String {
        text: String,
        source: ftd::TextSource,
    },
    Integer {
        value: i64,
    },
    Decimal {
        value: f64,
    },
    Boolean {
        value: bool,
    },
    Object {
        values: std::collections::BTreeMap<String, PropertyValue>,
    },
    Record {
        name: String,
        fields: std::collections::BTreeMap<String, PropertyValue>,
    },
    OrType {
        name: String,
        variant: String,
        fields: std::collections::BTreeMap<String, PropertyValue>,
    },
    List {
        data: Vec<Value>,
        kind: ftd::p2::Kind,
    },
    Optional {
        data: Box<Option<Value>>,
        kind: ftd::p2::Kind,
    },
    Map {
        data: std::collections::BTreeMap<String, Value>,
        kind: ftd::p2::Kind,
    },
    UI {
        name: String,
        kind: crate::p2::Kind,
        data: std::collections::BTreeMap<String, ftd::component::Property>,
    },
}

impl Value {
    pub fn is_null(&self) -> bool {
        if matches!(self, Self::None { .. }) {
            return true;
        }
        if let Self::Optional { data, .. } = self {
            if data.as_ref().eq(&None) {
                return true;
            }
        }
        false
    }

    pub fn is_empty(&self) -> bool {
        if let Self::List { data, .. } = self {
            if data.is_empty() {
                return true;
            }
        }
        false
    }

    pub fn kind(&self) -> ftd::p2::Kind {
        match self {
            Value::None { kind: k } => k.to_owned(),
            Value::String { source, .. } => ftd::p2::Kind::String {
                caption: *source == TextSource::Caption,
                body: *source == TextSource::Body,
                default: None,
            },
            Value::Integer { .. } => ftd::p2::Kind::integer(),
            Value::Decimal { .. } => ftd::p2::Kind::decimal(),
            Value::Boolean { .. } => ftd::p2::Kind::boolean(),
            Value::Object { .. } => ftd::p2::Kind::object(),
            Value::Record { name: id, .. } => ftd::p2::Kind::Record {
                name: id.to_string(),
                default: None,
            },
            Value::OrType { name: id, .. } => ftd::p2::Kind::OrType {
                name: id.to_string(),
            },
            Value::List { kind, .. } => ftd::p2::Kind::List {
                kind: Box::new(kind.to_owned()),
                default: None,
            },
            Value::Optional { kind, .. } => ftd::p2::Kind::Optional {
                kind: Box::new(kind.to_owned()),
            },
            Value::Map { kind, .. } => ftd::p2::Kind::Map {
                kind: Box::new(kind.to_owned()),
            },
            Value::UI { kind, .. } => kind.to_owned(),
        }
    }

    pub fn is_equal(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::String { text: a, .. }, Value::String { text: b, .. }) => a == b,
            (a, b) => a == b,
        }
    }

    pub fn to_string(&self) -> Option<String> {
        match self {
            Value::String { text, .. } => Some(text.to_string()),
            Value::Integer { value } => Some(value.to_string()),
            Value::Decimal { value } => Some(value.to_string()),
            Value::Boolean { value } => Some(value.to_string()),
            Value::Optional { data, .. } => {
                if let Some(data) = data.as_ref() {
                    data.to_string()
                } else {
                    Some("".to_string())
                }
            }
            Value::Object { values } => {
                let mut new_values: std::collections::BTreeMap<String, String> = Default::default();
                for (k, v) in values {
                    if let ftd::PropertyValue::Value { value } = v {
                        if let Some(v) = value.to_string() {
                            new_values.insert(k.to_owned(), v);
                        }
                    }
                }
                serde_json::to_string(&new_values).ok()
            }
            _ => None,
        }
    }
}

impl Variable {
    pub fn list_from_p1(p1: &ftd::p1::Section, doc: &ftd::p2::TDoc) -> ftd::p1::Result<Self> {
        let var_data = ftd::variable::VariableData::get_name_kind(
            &p1.name,
            doc,
            p1.line_number,
            vec![].as_slice(),
        )?;
        let name = doc.resolve_name(p1.line_number, &var_data.name)?;
        let kind = ftd::p2::Kind::for_variable(
            p1.line_number,
            &p1.name,
            None,
            doc,
            None,
            &Default::default(),
        )?;
        if !kind.is_list() {
            return ftd::e2(
                format!("Expected list found: {:?}", p1),
                doc.name,
                p1.line_number,
            );
        }
        Ok(Variable {
            name,
            value: Value::List {
                data: Default::default(),
                kind: kind.list_kind().to_owned(),
            },
            conditions: vec![],
        })
    }

    pub fn map_from_p1(p1: &ftd::p1::Section, doc: &ftd::p2::TDoc) -> ftd::p1::Result<Self> {
        let name = doc.resolve_name(
            p1.line_number,
            ftd::get_name("map", p1.name.as_str(), doc.name)?,
        )?;
        Ok(Variable {
            name,
            value: Value::Map {
                data: Default::default(),
                kind: ftd::p2::Kind::from(
                    p1.line_number,
                    p1.header.str(doc.name, p1.line_number, "type")?,
                    doc,
                    None,
                )?,
            },
            conditions: vec![],
        })
    }

    pub fn update_from_p1(
        &mut self,
        p1: &ftd::p1::Section,
        doc: &ftd::p2::TDoc,
    ) -> ftd::p1::Result<()> {
        fn read_value(
            line_number: usize,
            kind: &ftd::p2::Kind,
            p1: &ftd::p1::Section,
            doc: &ftd::p2::TDoc,
        ) -> ftd::p1::Result<ftd::Value> {
            Ok(match kind {
                ftd::p2::Kind::Integer { .. } => read_integer(p1, doc.name)?,
                ftd::p2::Kind::Decimal { .. } => read_decimal(p1, doc.name)?,
                ftd::p2::Kind::Boolean { .. } => read_boolean(p1, doc.name)?,
                ftd::p2::Kind::String { .. } => read_string(p1, doc.name)?,
                ftd::p2::Kind::Record { name, .. } => {
                    doc.get_record(line_number, name)?.create(p1, doc)?
                }
                _ => unimplemented!("{:?}", kind),
            })
        }

        let p1 = {
            let mut p1 = p1.clone();
            p1.name = if let Some(n) = p1.name.strip_prefix('$') {
                n.to_string()
            } else {
                p1.name
            };
            p1
        };

        match (&self.value.kind(), &mut self.value) {
            (ftd::p2::Kind::Record { name, .. }, _) => {
                self.value = doc.get_record(p1.line_number, name)?.create(&p1, doc)?
            }
            (ftd::p2::Kind::List { kind, .. }, ftd::Value::List { data, .. }) => {
                data.push(read_value(p1.line_number, kind, &p1, doc)?);
            }
            (ftd::p2::Kind::Optional { kind }, ftd::Value::Optional { data, .. }) => {
                *data = Box::new(read_value(p1.line_number, kind, &p1, doc).ok());
            }
            (ftd::p2::Kind::Map { .. }, _) => {
                return ftd::e2("unexpected map", doc.name, p1.line_number)
            }
            (k, _) => self.value = read_value(p1.line_number, k, &p1, doc)?,
        };

        Ok(())
    }

    pub fn from_p1(p1: &ftd::p1::Section, doc: &ftd::p2::TDoc) -> ftd::p1::Result<Self> {
        let var_data = ftd::variable::VariableData::get_name_kind(
            &p1.name,
            doc,
            p1.line_number,
            vec![].as_slice(),
        )?;
        if !var_data.is_variable() {
            return ftd::e2(
                format!("expected variable, found: {}", p1.name),
                doc.name,
                p1.line_number,
            );
        }
        let name = var_data.name.clone();

        if var_data.is_optional() && p1.caption.is_none() && p1.body.is_none() {
            let kind = ftd::p2::Kind::for_variable(
                p1.line_number,
                &p1.name,
                None,
                doc,
                None,
                &Default::default(),
            )?;
            return Ok(Variable {
                name,
                value: ftd::Value::Optional {
                    data: Box::new(None),
                    kind: kind.inner().to_owned(),
                },
                conditions: vec![],
            });
        }

        let value = {
            let mut value = match var_data.kind.as_str() {
                "string" => read_string(p1, doc.name)?,
                "integer" => read_integer(p1, doc.name)?,
                "decimal" => read_decimal(p1, doc.name)?,
                "boolean" => read_boolean(p1, doc.name)?,
                "object" => read_object(p1, doc)?,
                t => match doc.get_thing(p1.line_number, t)? {
                    ftd::p2::Thing::Record(r) => r.create(p1, doc)?,
                    ftd::p2::Thing::OrTypeWithVariant { e, variant } => {
                        e.create(p1, variant, doc)?
                    }
                    t => {
                        return ftd::e2(
                            format!("unexpected thing found: {:?}", t),
                            doc.name,
                            p1.line_number,
                        );
                    }
                },
            };
            if var_data.is_optional() {
                let kind = ftd::p2::Kind::for_variable(
                    p1.line_number,
                    &p1.name,
                    None,
                    doc,
                    None,
                    &Default::default(),
                )?;
                value = ftd::Value::Optional {
                    data: Box::new(Some(value)),
                    kind: kind.inner().to_owned(),
                };
            }
            value
        };

        Ok(Variable {
            name,
            value,
            conditions: vec![],
        })
    }

    pub fn get_value(
        &self,
        p1: &ftd::p1::Section,
        doc: &ftd::p2::TDoc,
    ) -> ftd::p1::Result<ftd::Value> {
        match self.value.kind() {
            ftd::p2::Kind::String { .. } => read_string(p1, doc.name),
            ftd::p2::Kind::Integer { .. } => read_integer(p1, doc.name),
            ftd::p2::Kind::Decimal { .. } => read_decimal(p1, doc.name),
            ftd::p2::Kind::Boolean { .. } => read_boolean(p1, doc.name),
            ftd::p2::Kind::Record { name, .. } => match doc.get_thing(p1.line_number, &name)? {
                ftd::p2::Thing::Record(r) => r.create(p1, doc),
                t => ftd::e2(
                    format!("expected record type, found: {:?}", t),
                    doc.name,
                    p1.line_number,
                ),
            },
            ftd::p2::Kind::OrType { name } => match doc.get_thing(p1.line_number, &name)? {
                ftd::p2::Thing::OrTypeWithVariant { e, variant } => e.create(p1, variant, doc),
                t => ftd::e2(
                    format!("expected or-type type, found: {:?}", t),
                    doc.name,
                    p1.line_number,
                ),
            },
            t => ftd::e2(
                format!("unexpected type found: {:?}", t),
                doc.name,
                p1.line_number,
            ),
        }
    }
}

pub fn guess_type(s: &str, is_body: bool) -> ftd::p1::Result<Value> {
    if is_body {
        return Ok(Value::String {
            text: s.to_string(),
            source: TextSource::Body,
        });
    }
    let caption = match s {
        "true" => return Ok(Value::Boolean { value: true }),
        "false" => return Ok(Value::Boolean { value: false }),
        v => v,
    };

    if let Ok(v) = caption.parse::<i64>() {
        return Ok(Value::Integer { value: v });
    }

    if let Ok(v) = caption.parse::<f64>() {
        return Ok(Value::Decimal { value: v });
    }

    Ok(Value::String {
        text: caption.to_string(),
        source: TextSource::Caption,
    })
}

fn read_string(p1: &ftd::p1::Section, doc_id: &str) -> ftd::p1::Result<Value> {
    match (&p1.caption, &p1.body_without_comment()) {
        (Some(c), Some(b)) => ftd::e2(
            format!("both caption: `{}` and body: `{}` present", c, b.1),
            doc_id,
            p1.line_number,
        ),
        (Some(caption), None) => Ok(Value::String {
            text: caption.to_string(),
            source: TextSource::Caption,
        }),
        (None, Some(body)) => Ok(Value::String {
            text: body.1.to_string(),
            source: TextSource::Body,
        }),
        (None, None) => ftd::e2(
            "either body or caption is required for string",
            doc_id,
            p1.line_number,
        ),
    }
}

fn read_integer(p1: &ftd::p1::Section, doc_id: &str) -> ftd::p1::Result<Value> {
    let caption = p1.caption(p1.line_number, doc_id)?;
    if let Ok(v) = caption.parse::<i64>() {
        return Ok(Value::Integer { value: v });
    }

    ftd::e2("not a valid integer", doc_id, p1.line_number)
}

fn read_decimal(p1: &ftd::p1::Section, doc_id: &str) -> ftd::p1::Result<Value> {
    let caption = p1.caption(p1.line_number, doc_id)?;
    if let Ok(v) = caption.parse::<f64>() {
        return Ok(Value::Decimal { value: v });
    }

    ftd::e2("not a valid float", doc_id, p1.line_number)
}

fn read_boolean(p1: &ftd::p1::Section, doc_id: &str) -> ftd::p1::Result<Value> {
    let caption = p1.caption(p1.line_number, doc_id)?;
    if let Ok(v) = caption.parse::<bool>() {
        return Ok(Value::Boolean { value: v });
    }

    ftd::e2("not a valid bool", doc_id, p1.line_number)
}

fn read_object(p1: &ftd::p1::Section, doc: &ftd::p2::TDoc) -> ftd::p1::Result<Value> {
    let mut values: std::collections::BTreeMap<String, PropertyValue> = Default::default();
    for (line_number, k, v) in p1.header.0.iter() {
        let line_number = line_number.to_owned();
        let value = if v.trim().starts_with('$') {
            ftd::PropertyValue::resolve_value(line_number, v, None, doc, &Default::default(), None)?
        } else if let Ok(v) = ftd::PropertyValue::resolve_value(
            line_number,
            v,
            Some(ftd::p2::Kind::decimal()),
            doc,
            &Default::default(),
            None,
        ) {
            v
        } else if let Ok(v) = ftd::PropertyValue::resolve_value(
            line_number,
            v,
            Some(ftd::p2::Kind::boolean()),
            doc,
            &Default::default(),
            None,
        ) {
            v
        } else if let Ok(v) = ftd::PropertyValue::resolve_value(
            line_number,
            v,
            Some(ftd::p2::Kind::integer()),
            doc,
            &Default::default(),
            None,
        ) {
            v
        } else {
            ftd::PropertyValue::resolve_value(
                line_number,
                v,
                Some(ftd::p2::Kind::string()),
                doc,
                &Default::default(),
                None,
            )?
        };
        values.insert(k.to_string(), value);
    }

    Ok(Value::Object { values })
}

#[derive(Debug, Clone)]
pub struct VariableData {
    pub name: String,
    pub kind: String,
    pub modifier: VariableModifier,
    pub type_: Type,
}

#[derive(Debug, Clone)]
pub enum VariableModifier {
    None,
    List,
    Optional,
}

#[derive(Debug, Clone)]
pub enum Type {
    Variable,
    Component,
}

impl VariableData {
    pub fn get_name_kind(
        s: &str,
        doc: &ftd::p2::TDoc,
        line_number: usize,
        var_types: &[String],
    ) -> ftd::p1::Result<VariableData> {
        if s.starts_with("record ")
            || s.starts_with("or-type ")
            || s.starts_with("map ")
            || s == "container"
        {
            return ftd::e2(
                format!("invalid declaration, found: `{}`", s),
                doc.name,
                line_number,
            );
        }
        let expr = s.split_whitespace().collect::<Vec<&str>>();
        if expr.len() > 4 || expr.len() <= 1 {
            return ftd::e2(
                format!("invalid declaration, found: `{}`", s),
                doc.name,
                line_number,
            );
        }
        let mut name = expr.get(1);
        let mut kind = expr.get(0).map(|k| k.to_string());
        let mut modifier = VariableModifier::None;
        if expr.len() == 4 {
            if expr.get(1).unwrap().eq(&"or") {
                kind = Some(expr[..3].join(" "));
                name = expr.get(3);
            } else {
                return ftd::e2(
                    format!("invalid variable or list declaration, found: `{}`", s),
                    doc.name,
                    line_number,
                );
            }
        } else if expr.len() == 3 {
            if expr.get(1).unwrap().eq(&"list") {
                modifier = VariableModifier::List;
                name = expr.get(2);
                kind = expr.get(0).map(|k| k.to_string());
            } else if expr.get(0).unwrap().eq(&"optional") {
                modifier = VariableModifier::Optional;
                name = expr.get(2);
                kind = expr.get(1).map(|k| k.to_string());
            } else {
                return ftd::e2(
                    format!("invalid variable or list declaration, found: `{}`", s),
                    doc.name,
                    line_number,
                );
            }
        }

        let var_kind = kind.ok_or(ftd::p1::Error::ParseError {
            message: format!("kind not found `{}`", s),
            doc_id: doc.name.to_string(),
            line_number,
        })?;

        let type_ = match var_kind.as_str() {
            "string" | "caption" | "body" | "body or caption" | "caption or body" | "integer"
            | "decimal" | "boolean" | "object" => Type::Variable,
            a if doc.get_record(line_number, a).is_ok()
                || doc.get_or_type(line_number, a).is_ok()
                || doc.get_or_type_with_variant(line_number, a).is_ok()
                || var_types.contains(&a.to_string()) =>
            {
                Type::Variable
            }
            _ => Type::Component,
        };

        Ok(VariableData {
            name: name
                .ok_or(ftd::p1::Error::ParseError {
                    message: format!("name not found `{}`", s),
                    doc_id: doc.name.to_string(),
                    line_number,
                })?
                .to_string(),
            kind: var_kind,
            modifier,
            type_,
        })
    }

    pub fn is_variable(&self) -> bool {
        matches!(self.type_, Type::Variable)
    }

    pub fn is_none(&self) -> bool {
        matches!(self.modifier, VariableModifier::None)
    }

    pub fn is_list(&self) -> bool {
        matches!(self.modifier, VariableModifier::List)
    }

    pub fn is_optional(&self) -> bool {
        matches!(self.modifier, VariableModifier::Optional)
    }
}
#[cfg(test)]
mod test {
    use ftd::test::*;

    macro_rules! p2 {
        ($s:expr, $n: expr, $v: expr, $c: expr,) => {
            p2!($s, $n, $v, $c)
        };
        ($s:expr, $n: expr, $v: expr, $c: expr) => {
            let p1 = ftd::p1::parse(indoc::indoc!($s), "foo").unwrap();
            let mut bag = std::collections::BTreeMap::new();
            let aliases = std::collections::BTreeMap::new();
            let mut d = ftd::p2::TDoc {
                name: "foo",
                bag: &mut bag,
                aliases: &aliases,
            };
            pretty_assertions::assert_eq!(
                super::Variable::from_p1(&p1[0], &mut d).unwrap(),
                super::Variable {
                    name: $n.to_string(),
                    value: $v,
                    conditions: $c
                }
            )
        };
    }

    #[test]
    fn int() {
        use super::Value::Integer;
        p2!("-- integer x: 10", "x", Integer { value: 10 }, vec![],);
    }

    #[test]
    fn float() {
        use super::Value::Decimal;
        p2!("-- decimal x: 10", "x", Decimal { value: 10.0 }, vec![],);
    }

    #[test]
    fn bool() {
        use super::Value::Boolean;
        p2!("-- boolean x: true", "x", Boolean { value: true }, vec![],);
        p2!("-- boolean x: false", "x", Boolean { value: false }, vec![],);
    }

    #[test]
    fn str() {
        use super::Value::String;
        p2!(
            "-- string x: hello",
            "x",
            String {
                text: "hello".to_string(),
                source: ftd::TextSource::Caption
            },
            vec![],
        );
        p2!(
            "-- string x:\n\nhello world\nyo!",
            "x",
            String {
                text: "hello world\nyo!".to_string(),
                source: ftd::TextSource::Body
            },
            vec![],
        );
        p2!(
            "-- string x: 10",
            "x",
            String {
                text: "10".to_string(),
                source: ftd::TextSource::Caption
            },
            vec![],
        );
        p2!(
            "-- string x: true",
            "x",
            String {
                text: "true".to_string(),
                source: ftd::TextSource::Caption
            },
            vec![],
        );
    }

    #[test]
    #[ignore]
    fn list_with_component() {
        let mut bag = default_bag();
        bag.insert(
            s("foo/bar#pull-request"),
            ftd::p2::Thing::Record(ftd::p2::Record {
                name: s("foo/bar#pull-request"),
                fields: std::array::IntoIter::new([
                    (s("title"), ftd::p2::Kind::caption()),
                    (s("about"), ftd::p2::Kind::body()),
                ])
                .collect(),
                instances: Default::default(),
                order: vec![s("title"), s("about")],
            }),
        );

        bag.insert(
            "foo/bar#pr".to_string(),
            ftd::p2::Thing::Variable(ftd::Variable {
                name: "foo/bar#pr".to_string(),
                value: ftd::Value::List {
                    data: vec![ftd::Value::Record {
                        name: s("foo/bar#pull-request"),
                        fields: std::array::IntoIter::new([
                            (
                                s("title"),
                                ftd::PropertyValue::Value {
                                    value: ftd::Value::String {
                                        text: "some pr".to_string(),
                                        source: ftd::TextSource::Caption,
                                    },
                                },
                            ),
                            (
                                s("about"),
                                ftd::PropertyValue::Value {
                                    value: ftd::Value::String {
                                        text: "yo yo".to_string(),
                                        source: ftd::TextSource::Body,
                                    },
                                },
                            ),
                        ])
                        .collect(),
                    }],
                    kind: ftd::p2::Kind::Record {
                        name: s("foo/bar#pull-request"),
                        default: None,
                    },
                },
                conditions: vec![],
            }),
        );

        p!(
            "
            -- record pull-request:
            caption title:
            body about:

            -- ftd.column pr-view:
            pull-request pr:

            --- ftd.text:
            text: $pr.title

            --- ftd.text:
            text: $pr.about

            -- list pr:
            type: pull-request

            -- pr: some pr

            yo yo
            ",
            (bag, default_column()),
        );
    }
}
