use core::cell::{Cell, RefCell};
use html5ever::{
    interface::QualName,
    namespace_url, ns,
    rcdom::{Handle, Node, NodeData, RcDom, WeakHandle},
    tendril::{fmt, Atomicity, NonAtomic, StrTendril, Tendril, TendrilSink},
    tree_builder::{Attribute, QuirksMode},
    LocalName, Namespace, Prefix,
};
use serde::{de, Deserialize, Deserializer, Serialize, Serializer};
use std::{
    borrow::Cow,
    io::{self, Write},
    rc::Rc,
};

macro_rules! transparent_wrapper {
    {
        $(#[$wrap_attr:meta])*
        $vis:vis struct $wrapper:ident($(#[$attr:meta])* $ty:ty);
    } => {
        $(#[$wrap_attr])*
        #[repr(transparent)]
        $vis struct $wrapper($(#[$attr])* pub $ty);
        #[allow(dead_code)]
        impl $wrapper {
            #[inline(always)]
            fn transmute_from(val: $ty) -> $wrapper {
                unsafe { core::mem::transmute::<$ty, Self>(val) }
            }
            #[inline(always)]
            fn transmute_into(self: $wrapper) -> $ty {
                unsafe { core::mem::transmute::<Self, $ty>(self) }
            }
        }
        impl<'a> From<&'a $ty> for &'a $wrapper {
            #[inline(always)]
            fn from(val: &'a $ty) -> Self {
                let ptr = val as *const $ty;
                unsafe { &*(ptr as *const $wrapper) }
            }
        }
        impl<'a> From<&'a $wrapper> for &'a $ty {
            #[inline(always)]
            fn from(val: &'a $wrapper) -> Self {
                let ptr = val as *const $wrapper;
                unsafe { &*(ptr as *const $ty) }
            }
        }
        impl<'a> From<&'a mut $ty> for &'a mut $wrapper {
            #[inline(always)]
            fn from(val: &'a mut $ty) -> Self {
                let ptr = val as *mut $ty;
                unsafe { &mut *(ptr as *mut $wrapper) }
            }
        }
        impl<'a> From<&'a mut $wrapper> for &'a mut $ty {
            #[inline(always)]
            fn from(val: &'a mut $wrapper) -> Self {
                let ptr = val as *mut $wrapper;
                unsafe { &mut *(ptr as *mut $ty) }
            }
        }
    };
}

// See the following for information on transmute weirdness:
// https://github.com/rust-lang/rust/issues/47966
// https://stackoverflow.com/questions/49722434/why-am-i-not-allowed-to-transmute-a-value-containing-a-traits-associated-type
// https://github.com/rust-lang/rust/pull/57044
macro_rules! helper_for_def {
    ($vis:vis $helper:ident = $def:ty as $ty:ty) => {
        transparent_wrapper! { $vis struct $helper($ty); }
        impl Serialize for $helper {
            #[inline(always)]
            fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                <$def>::serialize(<&Self as Into<&$ty>>::into(self), serializer)
            }
        }
        impl<'de> Deserialize<'de> for $helper {
            #[inline(always)]
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let val: Result<$ty, D::Error> = <$def>::deserialize(deserializer);
                val.map(|v| $helper::transmute_from(v))
            }
        }
    };
}
macro_rules! def_redir_helper {
    ($vis:vis $helper:ident = $ty:ty => $def:ty) => {
        transparent_wrapper! { $vis struct $helper($ty); }
        impl $helper {
            #[inline(always)]
            fn serialize<S: Serializer>(val: &$ty, serializer: S) -> Result<S::Ok, S::Error> {
                let ptr = val as *const $ty;
                let def: &$def = unsafe { &*(ptr as *const $def) };
                def.serialize(serializer)
            }
            #[inline(always)]
            fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<$ty, D::Error> {
                let def: Result<$def, D::Error> = <$def as Deserialize>::deserialize(deserializer);
                def.map(|d| unsafe { core::mem::transmute::<$def, $ty>(d) })
            }
        }
    };
}

fn is_false(val: &bool) -> bool {
    !val
}

fn is_empty_refcell_vec<T>(val: &RefCell<Vec<T>>) -> bool {
    val.borrow().is_empty()
}

fn is_empty_tendril<F: fmt::Format, A: Atomicity>(val: &Tendril<F, A>) -> bool {
    val.len32() == 0
}

#[derive(Deserialize, Serialize)]
#[serde(remote = "Attribute")]
pub struct AttributeDef {
    #[serde(with = "QualNameHelper")]
    pub name: QualName,
    #[serde(with = "StrTendrilDef")]
    pub value: StrTendril,
}

helper_for_def!(pub AttributeHelper = AttributeDef as Attribute);
def_redir_helper!(pub RefCellVecAttributeDef = RefCell<Vec<Attribute>> => RefCell<Vec<AttributeHelper>>);

#[derive(Deserialize, Serialize)]
#[serde(remote = "Node")]
pub struct NodeDef {
    #[serde(skip)]
    pub parent: Cell<Option<WeakHandle>>,
    #[serde(flatten, with = "NodeDataDef")]
    pub data: NodeData,
    #[serde(
        skip_serializing_if = "is_empty_refcell_vec",
        with = "RefCellVecHandleDef"
    )]
    pub children: RefCell<Vec<Handle>>,
}

helper_for_def!(pub NodeHelper = NodeDef as Node);
pub type HandleHelper = Rc<NodeHelper>;
def_redir_helper!(pub HandleDef = Handle => HandleHelper);
def_redir_helper!(pub OptionHandleDef = Option<Handle> => Option<HandleHelper>);
def_redir_helper!(pub RefCellVecHandleDef = RefCell<Vec<Handle>> => RefCell<Vec<HandleHelper>>);

#[derive(Deserialize, Serialize)]
#[serde(remote = "NodeData", tag = "type")]
pub enum NodeDataDef {
    Document,
    Doctype {
        #[serde(with = "StrTendrilDef")]
        name: StrTendril,
        #[serde(skip_serializing_if = "is_empty_tendril", with = "StrTendrilDef")]
        public_id: StrTendril,
        #[serde(skip_serializing_if = "is_empty_tendril", with = "StrTendrilDef")]
        system_id: StrTendril,
    },
    Text {
        #[serde(with = "RefCellStrTendrilDef")]
        contents: RefCell<StrTendril>,
    },
    Comment {
        #[serde(with = "StrTendrilDef")]
        contents: StrTendril,
    },
    Element {
        #[serde(with = "QualNameHelper")]
        name: QualName,
        #[serde(
            skip_serializing_if = "is_empty_refcell_vec",
            with = "RefCellVecAttributeDef"
        )]
        attrs: RefCell<Vec<Attribute>>,
        #[serde(skip_serializing_if = "Option::is_none", with = "OptionHandleDef")]
        template_contents: Option<Handle>,
        #[serde(skip_serializing_if = "is_false")]
        mathml_annotation_xml_integration_point: bool,
    },
    ProcessingInstruction {
        #[serde(with = "StrTendrilDef")]
        target: StrTendril,
        #[serde(with = "StrTendrilDef")]
        contents: StrTendril,
    },
}

#[derive(Deserialize, Serialize)]
#[serde(remote = "QualName")]
pub struct QualNameDef {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prefix: Option<Prefix>,
    #[serde(skip_serializing_if = "str::is_empty")]
    pub ns: Namespace,
    pub local: LocalName,
}

pub struct QualNameHelper(pub QualName);
impl QualNameHelper {
    pub fn serialize<S: Serializer>(val: &QualName, serializer: S) -> Result<S::Ok, S::Error> {
        match val {
            QualName {
                prefix: None,
                ns: ns!(html),
                local,
            } => local.serialize(serializer),
            val => QualNameDef::serialize(val, serializer),
        }
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<QualName, D::Error> {
        struct StringOrStruct();

        impl<'de> de::Visitor<'de> for StringOrStruct {
            type Value = QualName;

            fn expecting(&self, formatter: &mut core::fmt::Formatter) -> core::fmt::Result {
                formatter.write_str("string or map")
            }

            fn visit_str<E: de::Error>(self, value: &str) -> Result<QualName, E> {
                Ok(QualName::new(None, ns!(html), value.into()))
            }

            fn visit_map<M: de::MapAccess<'de>>(self, map: M) -> Result<QualName, M::Error> {
                QualNameDef::deserialize(de::value::MapAccessDeserializer::new(map))
            }
        }

        deserializer.deserialize_any(StringOrStruct())
    }
}

#[derive(Deserialize, Serialize)]
#[serde(remote = "QuirksMode")]
pub enum QuirksModeDef {
    Quirks,
    LimitedQuirks,
    NoQuirks,
}
impl QuirksModeDef {
    pub fn lacks_quirks(val: &QuirksMode) -> bool {
        match val {
            QuirksMode::NoQuirks => true,
            _ => false,
        }
    }
}

#[derive(Deserialize, Serialize)]
#[serde(remote = "RcDom")]
pub struct RcDomDef {
    #[serde(
        skip_serializing_if = "QuirksModeDef::lacks_quirks",
        with = "QuirksModeDef"
    )]
    pub quirks_mode: QuirksMode,
    #[serde(with = "HandleDef")]
    pub document: Handle,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub errors: Vec<Cow<'static, str>>,
}

helper_for_def!(pub RcDomHelper = RcDomDef as RcDom);

macro_rules! tendril_def {
    ($vis:vis $def:ident<$fmt:ty, $atomicity:ty> -> $slice:ty, $serialize:expr) => {
        $vis struct $def(pub Tendril<$fmt, $atomicity>);
        impl $def {
            fn serialize<S: Serializer>(
                val: &Tendril<$fmt, $atomicity>,
                serializer: S,
            ) -> Result<S::Ok, S::Error> {
                $serialize(serializer, &*val)
            }
            fn deserialize<'de, D: Deserializer<'de>>(
                deserializer: D,
            ) -> Result<Tendril<$fmt, $atomicity>, D::Error> {
                <&$slice>::deserialize(deserializer).map(Tendril::from_slice)
            }
        }
    };
}
tendril_def!(pub StrTendrilDef<fmt::UTF8, NonAtomic> -> str, Serializer::serialize_str);
// tendril_def!(pub ByteTendrilDef<fmt::Bytes, NonAtomic> -> [u8], Serializer::serialize_bytes);
helper_for_def!(pub StrTendrilHelper = StrTendrilDef as StrTendril);
def_redir_helper!(pub RefCellStrTendrilDef = RefCell<StrTendril> => RefCell<StrTendrilHelper>);

fn main() -> io::Result<()> {
    let dom: RcDom = html5ever::parse_document(RcDom::default(), html5ever::ParseOpts::default())
        .from_utf8()
        .read_from(&mut io::stdin().lock())?;

    writeln!(
        &mut io::stdout().lock(),
        "{}",
        serde_json::to_string(<&RcDomHelper>::from(&dom))?
    )?;

    Ok(())
}
