// libreg ~ (peiosutils) — the registry's batch document, for reg and for
// programs.
//
// A document is a run of keys, each with its path and values, in JSON
// (docs/reg-spec.md §6): what `reg export --json` writes and `reg apply`
// reads, and what Registry Editor exports and imports. `export` reads a key
// and everything under it into one; `apply` writes one back in a single
// transaction, all or nothing.
//
// A value's data is JSON of its type: a string, a number, a list of strings,
// or hex for bytes. Data that doesn't fit its type (a string that isn't
// UTF-8, a number of the wrong length) or of a type with no JSON form is
// written as `hex`, the bytes themselves, beside its type, so that every
// value survives an export and an apply exactly.

use std::fmt;

use peios::registry::{
    CreateFlags, Data, Key, KeyAccess, OpenFlags, SecInfo, Transaction, ValueType,
};
use peios::security::{SecurityDescriptor, sddl};
use serde::{Deserialize, Serialize};
use serde_json::{Value as Json, json};

/// A batch document: keys, parents before the keys under them.
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct Document {
    #[serde(default)]
    pub keys: Vec<KeyEntry>,
}

/// One key of a document.
///
/// Unknown fields are refused rather than ignored: a field this version
/// does not know is one it would otherwise drop while reporting success,
/// as versions before `descriptor` silently dropped that.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KeyEntry {
    pub path: String,
    #[serde(default)]
    pub values: Vec<ValueEntry>,
    /// Security descriptor parts to set on the key, as SDDL. Only the parts
    /// the SDDL gives are set (`S:` alone sets the SACL and leaves the
    /// inherited owner, group and DACL as they are). `export` never fills
    /// it: reading a SACL needs a privilege an export should not demand.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub descriptor: Option<String>,
}

/// The parts of `sddl` to set, and the descriptor holding them: owner,
/// group, DACL and SACL, each where the SDDL gives it. An SDDL that gives
/// none is refused, since applying it would change nothing while looking
/// as though it had.
pub fn descriptor_parts(sddl: &str) -> Result<(SecInfo, SecurityDescriptor), Error> {
    let invalid = |why: String| Error::Invalid(format!("descriptor {sddl:?}: {why}"));
    let sd = sddl::parse(sddl).map_err(|e| invalid(e.to_string()))?;
    let view = sd.view().map_err(|e| invalid(e.to_string()))?;
    let mut parts = SecInfo::empty();
    if view.owner().is_some() {
        parts |= SecInfo::OWNER;
    }
    if view.group().is_some() {
        parts |= SecInfo::GROUP;
    }
    if view.dacl().is_some() {
        parts |= SecInfo::DACL;
    }
    if view.sacl().is_some() {
        parts |= SecInfo::SACL;
    }
    if parts.is_empty() {
        return Err(invalid("it gives no owner, group, DACL or SACL".into()));
    }
    Ok((parts, sd))
}

/// The rights a handle needs to set `parts` of a key's descriptor.
pub fn descriptor_access(parts: SecInfo) -> KeyAccess {
    let mut access = KeyAccess::empty();
    if parts.intersects(SecInfo::OWNER | SecInfo::GROUP) {
        access |= KeyAccess::WRITE_OWNER;
    }
    if parts.intersects(SecInfo::DACL | SecInfo::LABEL) {
        access |= KeyAccess::WRITE_DAC;
    }
    if parts.contains(SecInfo::SACL) {
        access |= KeyAccess::ACCESS_SYSTEM_SECURITY;
    }
    access
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ValueEntry {
    /// The value's name; `@` for the key's default value.
    pub name: String,
    /// Its type's keyword (`sz`, `dword`, …), or `0x…` for a type with none.
    #[serde(rename = "type")]
    pub ty: String,
    /// Its data, as JSON of its type.
    #[serde(default, skip_serializing_if = "Json::is_null")]
    pub data: Json,
    /// Its bytes in hex, where `data` can't hold them exactly.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hex: Option<String>,
}

impl ValueEntry {
    /// The entry for the value `name` holding `bytes` as `ty`.
    pub fn of(name: &[u8], ty: ValueType, bytes: &[u8]) -> Self {
        let name = if name.is_empty() {
            "@".to_string()
        } else {
            String::from_utf8_lossy(name).into_owned()
        };
        match data_json(ty, bytes) {
            Some(data) => Self {
                name,
                ty: keyword(ty),
                data,
                hex: None,
            },
            None => Self {
                name,
                ty: keyword(ty),
                data: Json::Null,
                hex: Some(hex(bytes)),
            },
        }
    }

    /// The value's name as bytes, and its type and bytes.
    pub fn bytes(&self) -> Result<(Vec<u8>, ValueType, Vec<u8>), Error> {
        let name = if self.name == "@" {
            Vec::new()
        } else {
            self.name.as_bytes().to_vec()
        };
        let ty = type_of(&self.ty)
            .ok_or_else(|| Error::Invalid(format!("unknown value type: {}", self.ty)))?;
        let bytes = match &self.hex {
            Some(hex) => unhex(hex)?,
            None => from_json(ty, &self.ty, &self.data)?,
        };
        Ok((name, ty, bytes))
    }
}

/// What went wrong, and on what.
#[derive(Debug)]
pub enum Error {
    /// A registry call failed: what was being done, on which key.
    Registry {
        op: &'static str,
        path: String,
        source: peios::Error,
    },
    /// The document isn't one: bad JSON, a type, or data that doesn't fit it.
    Invalid(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Registry { op, path, source } => write!(f, "{op} {path}: {source}"),
            Self::Invalid(why) => f.write_str(why),
        }
    }
}

impl std::error::Error for Error {}

fn registry<'a>(op: &'static str, path: &'a str) -> impl FnOnce(peios::Error) -> Error + 'a {
    move |source| Error::Registry {
        op,
        path: path.to_string(),
        source,
    }
}

/// The key at `path` and everything under it, as a document: each key with
/// its values, depth first, parents first. A key's values that may not be
/// read are left out; a key whose subkeys may not be listed is an error.
pub fn export(path: &str) -> Result<Document, Error> {
    let mut doc = Document::default();
    collect(path, &mut doc)?;
    Ok(doc)
}

fn collect(path: &str, doc: &mut Document) -> Result<(), Error> {
    let key = Key::open(None, path, KeyAccess::READ, OpenFlags::empty())
        .map_err(registry("open key", path))?;
    let values = key
        .query_values_batch(None)
        .map(|records| {
            records
                .iter()
                .filter(|r| r.ty != ValueType::TOMBSTONE)
                .map(|r| ValueEntry::of(&r.name, r.ty, &r.data))
                .collect()
        })
        .unwrap_or_default();
    doc.keys.push(KeyEntry {
        path: path.to_string(),
        values,
        descriptor: None,
    });
    let children = key
        .subkeys(None)
        .map(|subkey| subkey.map(|subkey| String::from_utf8_lossy(&subkey.name).into_owned()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(registry("list subkeys of", path))?;
    for child in children {
        collect(&format!("{path}\\{child}"), doc)?;
    }
    Ok(())
}

/// Writes `doc` in one transaction, into `layer` (`None`: the base layer),
/// all or nothing: each key is created if it isn't there, parents first,
/// each value set, and then the parts of its `descriptor` the SDDL gives.
/// Every key is created inside the transaction, so no open waits behind its
/// own writes (PEI-1241). A descriptor's SACL needs SeSecurityPrivilege
/// enabled, as `reg sd --sacl` does. Says how many keys.
pub fn apply(doc: &Document, layer: Option<&str>) -> Result<usize, Error> {
    // Every value and descriptor checked before anything is written.
    let entries = doc
        .keys
        .iter()
        .map(|entry| {
            Ok((
                entry,
                entry
                    .values
                    .iter()
                    .map(ValueEntry::bytes)
                    .collect::<Result<Vec<_>, Error>>()?,
                entry
                    .descriptor
                    .as_deref()
                    .map(descriptor_parts)
                    .transpose()?,
            ))
        })
        .collect::<Result<Vec<_>, Error>>()?;
    let txn = Transaction::begin().map_err(registry("begin a transaction for", ""))?;
    // Held open until the commit, so what is enlisted stays valid.
    let mut keys = Vec::new();
    for (entry, values, descriptor) in entries {
        let path = entry.path.replace('/', "\\");
        let access = KeyAccess::WRITE
            | KeyAccess::SET_VALUE
            | descriptor
                .as_ref()
                .map_or(KeyAccess::empty(), |(parts, _)| descriptor_access(*parts));
        let (key, _) = Key::create(None, &path, access, CreateFlags::empty(), layer, Some(&txn))
            .map_err(registry("create key", &entry.path))?;
        for (name, ty, bytes) in values {
            let mut write = key.set_value(&name, ty, &bytes);
            if let Some(layer) = layer {
                write.layer(layer);
            }
            write
                .in_txn(&txn)
                .call()
                .map_err(registry("set a value of", &entry.path))?;
        }
        // On the key this handle names, in the same transaction: the
        // descriptor commits with the values or not at all.
        if let Some((parts, sd)) = &descriptor {
            key.set_security(*parts, sd, Some(&txn))
                .map_err(registry("set the descriptor of", &entry.path))?;
        }
        keys.push(key);
    }
    txn.commit()
        .map_err(registry("commit the transaction for", ""))?;
    drop(keys);
    Ok(doc.keys.len())
}

/// A type's keyword in a document: `sz`, `dword`, …, or `0x…` for a type
/// with none.
pub fn keyword(ty: ValueType) -> String {
    match ty {
        ValueType::NONE => "none".into(),
        ValueType::SZ => "sz".into(),
        ValueType::EXPAND_SZ => "expand".into(),
        ValueType::BINARY => "binary".into(),
        ValueType::DWORD => "dword".into(),
        ValueType::DWORD_BIG_ENDIAN => "dword-be".into(),
        ValueType::LINK => "link".into(),
        ValueType::MULTI_SZ => "multi".into(),
        ValueType::QWORD => "qword".into(),
        ValueType::TOMBSTONE => "tombstone".into(),
        other => format!("{:#x}", other.0),
    }
}

/// The type a keyword names.
pub fn type_of(keyword: &str) -> Option<ValueType> {
    Some(match keyword {
        "none" => ValueType::NONE,
        "sz" => ValueType::SZ,
        "expand" => ValueType::EXPAND_SZ,
        "binary" => ValueType::BINARY,
        "dword" => ValueType::DWORD,
        "dword-be" => ValueType::DWORD_BIG_ENDIAN,
        "link" => ValueType::LINK,
        "multi" => ValueType::MULTI_SZ,
        "qword" => ValueType::QWORD,
        "tombstone" => ValueType::TOMBSTONE,
        other => ValueType(u32::from_str_radix(other.strip_prefix("0x")?, 16).ok()?),
    })
}

/// `bytes` of type `ty` as JSON of its type, or `None` where JSON can't hold
/// them exactly.
pub fn data_json(ty: ValueType, bytes: &[u8]) -> Option<Json> {
    Some(match Data::decode(ty, bytes) {
        Data::Sz(s) | Data::ExpandSz(s) | Data::Link(s) => json!(s),
        Data::Dword(n) | Data::DwordBigEndian(n) => json!(n),
        Data::Qword(n) => json!(n),
        Data::MultiSz(list) => json!(list),
        Data::Binary(bytes) => json!(hex(&bytes)),
        Data::None => Json::Null,
        Data::Raw(..) => return None,
    })
}

/// The bytes JSON `data` stands for, as type `ty`, named `keyword`.
fn from_json(ty: ValueType, keyword: &str, data: &Json) -> Result<Vec<u8>, Error> {
    let string = || {
        data.as_str()
            .map(str::to_string)
            .ok_or_else(|| Error::Invalid(format!("{keyword}: expected a JSON string")))
    };
    let number = || {
        data.as_u64()
            .ok_or_else(|| Error::Invalid(format!("{keyword}: expected a non-negative integer")))
    };
    let small = |n: u64| {
        u32::try_from(n).map_err(|_| Error::Invalid(format!("{keyword}: value does not fit u32")))
    };
    let data = match ty {
        ValueType::SZ => Data::Sz(string()?),
        ValueType::EXPAND_SZ => Data::ExpandSz(string()?),
        ValueType::LINK => Data::Link(string()?),
        ValueType::DWORD => Data::Dword(small(number()?)?),
        ValueType::DWORD_BIG_ENDIAN => Data::DwordBigEndian(small(number()?)?),
        ValueType::QWORD => Data::Qword(number()?),
        ValueType::MULTI_SZ => {
            let list = data
                .as_array()
                .ok_or_else(|| Error::Invalid("multi: expected a JSON array".into()))?;
            Data::MultiSz(
                list.iter()
                    .map(|item| {
                        item.as_str()
                            .map(str::to_string)
                            .ok_or_else(|| Error::Invalid("multi: expected strings".into()))
                    })
                    .collect::<Result<_, _>>()?,
            )
        }
        ValueType::BINARY => Data::Binary(unhex(&string()?)?),
        ValueType::NONE | ValueType::TOMBSTONE => return Ok(Vec::new()),
        _ => return Err(Error::Invalid(format!("{keyword}: give its bytes as hex"))),
    };
    Ok(data.encode())
}

/// Lowercase hex with no separators.
pub fn hex(bytes: &[u8]) -> String {
    use fmt::Write;
    bytes
        .iter()
        .fold(String::with_capacity(bytes.len() * 2), |mut out, b| {
            let _ = write!(out, "{b:02x}");
            out
        })
}

/// Bytes from hex, any of `:`, `-` and whitespace between them ignored.
pub fn unhex(text: &str) -> Result<Vec<u8>, Error> {
    let digits: String = text
        .chars()
        .filter(|c| !c.is_whitespace() && !matches!(c, ':' | '-'))
        .collect();
    if !digits.len().is_multiple_of(2) || !digits.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(Error::Invalid("hex: bad or odd-length hex".into()));
    }
    Ok((0..digits.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(&digits[at..at + 2], 16).unwrap_or_default())
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round(ty: ValueType, bytes: &[u8]) -> (ValueEntry, (Vec<u8>, ValueType, Vec<u8>)) {
        let entry = ValueEntry::of(b"V", ty, bytes);
        let json = serde_json::to_string(&entry).unwrap();
        let back: ValueEntry = serde_json::from_str(&json).unwrap();
        let bytes = back.bytes().unwrap();
        (entry, bytes)
    }

    #[test]
    fn every_value_survives_a_document_exactly() {
        for (ty, bytes) in [
            (ValueType::SZ, b"dark\0".to_vec()),
            (ValueType::EXPAND_SZ, b"%HOME%\0".to_vec()),
            (ValueType::LINK, br"Machine\System".to_vec()),
            (ValueType::MULTI_SZ, b"a\0b\0\0".to_vec()),
            (ValueType::DWORD, 4096u32.to_le_bytes().to_vec()),
            (ValueType::DWORD_BIG_ENDIAN, 1u32.to_be_bytes().to_vec()),
            (ValueType::QWORD, 9_000_000_000u64.to_le_bytes().to_vec()),
            (ValueType::BINARY, vec![0xde, 0xad]),
            (ValueType::NONE, Vec::new()),
            // What JSON of its type can't hold goes as hex, exactly.
            (ValueType::SZ, b"\xff\0".to_vec()),
            (ValueType::DWORD, vec![1, 2, 3]),
            (ValueType::NONE, vec![7]),
            (ValueType(0x99), vec![1, 2]),
            (ValueType::RESOURCE_LIST, vec![9]),
        ] {
            let (entry, (_, back_ty, back)) = round(ty, &bytes);
            assert_eq!((back_ty, back), (ty, bytes.clone()), "{entry:?}");
        }
    }

    #[test]
    fn data_is_json_of_its_type_and_hex_only_where_it_must_be() {
        assert_eq!(
            ValueEntry::of(b"", ValueType::DWORD, &7u32.to_le_bytes()),
            ValueEntry {
                name: "@".into(),
                ty: "dword".into(),
                data: json!(7),
                hex: None
            }
        );
        let bad = ValueEntry::of(b"S", ValueType::SZ, b"\xff\0");
        assert_eq!(
            (bad.ty.as_str(), bad.hex.as_deref(), &bad.data),
            ("sz", Some("ff00"), &Json::Null)
        );
        assert_eq!(
            serde_json::to_string(&bad).unwrap(),
            r#"{"name":"S","type":"sz","hex":"ff00"}"#
        );
        assert_eq!(keyword(ValueType(0x99)), "0x99");
        assert_eq!(type_of("0x99"), Some(ValueType(0x99)));
    }

    #[test]
    fn a_document_written_by_an_older_reg_is_read() {
        let doc: Document = serde_json::from_str(r#"{"keys":[{"path":"Machine\\App","values":[{"name":"N","type":"dword","data":5},{"name":"@","type":"none","data":null}]}]}"#).unwrap();
        let values: Vec<_> = doc.keys[0]
            .values
            .iter()
            .map(|v| v.bytes().unwrap())
            .collect();
        assert_eq!(
            values[0],
            (b"N".to_vec(), ValueType::DWORD, 5u32.to_le_bytes().to_vec())
        );
        assert_eq!(values[1], (Vec::new(), ValueType::NONE, Vec::new()));
    }

    #[test]
    fn a_key_carries_a_descriptor_only_when_given_one() {
        let doc: Document = serde_json::from_str(
            r#"{"keys":[{"path":"Machine\\Generic"},{"path":"Machine\\Generic\\Events","descriptor":"S:(AL;CI;0x10002;;;WD)"}]}"#,
        )
        .unwrap();
        assert_eq!(doc.keys[0].descriptor, None);
        assert_eq!(
            doc.keys[1].descriptor.as_deref(),
            Some("S:(AL;CI;0x10002;;;WD)")
        );
        // An export, which never fills it, writes no field at all.
        assert_eq!(
            serde_json::to_string(&doc.keys[0]).unwrap(),
            r#"{"path":"Machine\\Generic","values":[]}"#
        );
    }

    #[test]
    fn an_unknown_key_field_is_refused_not_dropped() {
        let typo = serde_json::from_str::<Document>(
            r#"{"keys":[{"path":"Machine\\App","descripter":"S:(AL;CI;0x10002;;;WD)"}]}"#,
        );
        assert!(typo.unwrap_err().to_string().contains("descripter"));
        // The document's own top level still takes a comment.
        assert!(serde_json::from_str::<Document>(r#"{"_comment":["x"],"keys":[]}"#).is_ok());
    }

    #[test]
    fn only_the_descriptor_parts_given_are_set() {
        let (parts, _) = descriptor_parts("S:(AL;CI;0x10002;;;WD)").unwrap();
        assert_eq!(parts, SecInfo::SACL);
        assert_eq!(descriptor_access(parts), KeyAccess::ACCESS_SYSTEM_SECURITY);
        let (parts, _) = descriptor_parts("O:SYD:(A;;KA;;;SY)").unwrap();
        assert_eq!(parts, SecInfo::OWNER | SecInfo::DACL);
        assert_eq!(
            descriptor_access(parts),
            KeyAccess::WRITE_OWNER | KeyAccess::WRITE_DAC
        );
    }

    #[test]
    fn a_descriptor_that_sets_nothing_or_does_not_parse_is_refused() {
        assert!(descriptor_parts("").is_err());
        assert!(descriptor_parts("S:(nonsense)").is_err());
    }

    #[test]
    fn bad_data_is_refused_before_anything_is_written() {
        let entry = ValueEntry {
            name: "N".into(),
            ty: "dword".into(),
            data: json!(5_000_000_000u64),
            hex: None,
        };
        assert!(
            entry
                .bytes()
                .unwrap_err()
                .to_string()
                .contains("does not fit u32")
        );
        let entry = ValueEntry {
            name: "N".into(),
            ty: "nonsense".into(),
            data: Json::Null,
            hex: None,
        };
        assert!(entry.bytes().is_err());
    }
}
