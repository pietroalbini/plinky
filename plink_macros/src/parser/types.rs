use proc_macro::Span;

#[derive(Debug, Clone)]
pub(crate) enum Item {
    Struct(Struct),
    Enum(Enum),
}

#[derive(Debug, Clone)]
pub(crate) struct Struct {
    pub(crate) attrs: Vec<Attribute>,
    pub(crate) name: String,
    pub(crate) generics: Vec<GenericParam>,
    pub(crate) fields: StructFields,
    pub(crate) span: Span,
}

#[derive(Debug, Clone)]
pub(crate) enum StructFields {
    None,
    TupleLike(Vec<TupleField>),
    StructLike(Vec<StructField>),
}

#[derive(Debug, Clone)]
pub(crate) struct StructField {
    pub(crate) attrs: Vec<Attribute>,
    pub(crate) name: String,
    pub(crate) ty: String,
}

#[derive(Debug, Clone)]
pub(crate) struct Attribute {
    pub(crate) span: Span,
    pub(crate) value: String,
}

#[derive(Debug, Clone)]
pub(crate) struct Enum {
    pub(crate) _attrs: Vec<Attribute>,
    pub(crate) name: String,
    pub(crate) generics: Vec<GenericParam>,
    pub(crate) variants: Vec<EnumVariant>,
}

#[derive(Debug, Clone)]
pub(crate) struct EnumVariant {
    pub(crate) span: Span,
    pub(crate) attrs: Vec<Attribute>,
    pub(crate) name: String,
    pub(crate) data: EnumVariantData,
}

#[derive(Debug, Clone)]
pub(crate) enum EnumVariantData {
    None,
    TupleLike(Vec<TupleField>),
    StructLike(Vec<StructField>),
}

#[derive(Debug, Clone)]
pub(crate) struct TupleField {
    pub(crate) attrs: Vec<Attribute>,
    pub(crate) ty: String,
}

#[derive(Debug, Clone)]
pub(crate) enum GenericParam {
    Normal(GenericParamNormal),
    Const(GenericParamConst),
}

#[derive(Debug, Clone)]
pub(crate) struct GenericParamNormal {
    pub(crate) name: String,
    pub(crate) bound: String,
}

#[derive(Debug, Clone)]
pub(crate) struct GenericParamConst {
    pub(crate) name: String,
    pub(crate) type_: String,
    pub(crate) _default: Option<String>,
}
