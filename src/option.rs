//! Configurable options which can be used to customize behaviors or responses.
//!
//! # Note
//! Some requests do not support all of these parameters,
//! and using them will cause an error.
//!
//! Be careful and read the document of the requests
//! from Microsoft first.
//!
//! # See also
//! [Microsoft Docs](https://docs.microsoft.com/en-us/graph/query-parameters)
use crate::resource::{ResourceField, Tag};
use crate::util::{RequestBuilderExt, RequestBuilderTransformer};
use crate::ConflictBehavior;
use reqwest::{header, RequestBuilder};
use std::default::Default;
use std::fmt::Write;
use std::marker::PhantomData;

#[derive(Debug, Default)]
struct AccessOption {
    if_match: Option<String>,
    if_none_match: Option<String>,
}

impl AccessOption {
    fn if_match(mut self, tag: &Tag) -> Self {
        self.if_match = Some(tag.as_str().to_owned());
        self
    }

    fn if_none_match(mut self, tag: &Tag) -> Self {
        self.if_none_match = Some(tag.as_str().to_owned());
        self
    }
}

impl RequestBuilderTransformer for AccessOption {
    fn trans(&self, req: RequestBuilder) -> RequestBuilder {
        req.opt_header(header::IF_MATCH, self.if_match.as_ref())
            .opt_header(header::IF_NONE_MATCH, self.if_none_match.as_ref())
    }
}

/// Option for GET-like requests to one resource object.
#[derive(Debug)]
pub struct ObjectOption<Field> {
    access_opt: AccessOption,
    select_buf: String,
    expand_buf: String,
    _marker: PhantomData<Fn(&Field)>,
}

impl<Field: ResourceField> ObjectOption<Field> {
    /// Create an empty (default) option.
    pub fn new() -> Self {
        Self {
            access_opt: Default::default(),
            select_buf: String::new(),
            expand_buf: String::new(),
            _marker: PhantomData,
        }
    }

    /// Only response if the object matches the `tag`.
    ///
    /// Will cause HTTP 412 Precondition Failed otherwise.
    ///
    /// It is usually used for PUT-like requests to assert preconditions, but
    /// most of GET-like requests also support it.
    ///
    /// It will add `If-Match` to the request header.
    pub fn if_match(mut self, tag: &Tag) -> Self {
        self.access_opt = self.access_opt.if_match(tag);
        self
    }

    /// Only response if the object does not match the `tag`.
    ///
    /// Will cause the relative API returns `None` otherwise.
    ///
    /// It is usually used for GET-like requests to reduce data transmission if
    /// cached data can be reused.
    ///
    /// This will add `If-None-Match` to the request header.
    pub fn if_none_match(mut self, tag: &Tag) -> Self {
        self.access_opt = self.access_opt.if_none_match(tag);
        self
    }

    /// Select only some fields of the resource object.
    ///
    /// # Note
    /// If called more than once, all fields mentioned will be selected.
    ///
    /// # See also
    /// [Microsoft Docs](https://docs.microsoft.com/en-us/graph/query-parameters#select-parameter)
    pub fn select(mut self, fields: &[Field]) -> Self {
        for sel in fields {
            self = self.select_raw(&[sel.api_field_name()]);
        }
        self
    }

    fn select_raw(mut self, fields: &[&str]) -> Self {
        for sel in fields {
            write!(self.select_buf, ",{}", sel).unwrap();
        }
        self
    }

    /// Expand a field of the resource object.
    ///
    /// # Note
    /// If called more than once, all fields mentioned will be expanded.
    ///
    /// # See also
    /// [Microsoft Docs](https://docs.microsoft.com/en-us/graph/query-parameters#expand-parameter)
    pub fn expand(self, field: Field, select_children: Option<&[&str]>) -> Self {
        self.expand_raw(field.api_field_name(), select_children)
    }

    fn expand_raw(mut self, field: &str, select_children: Option<&[&str]>) -> Self {
        let buf = &mut self.expand_buf;
        write!(buf, ",{}", field).unwrap();
        if let Some(children) = select_children {
            write!(buf, "($select=").unwrap();
            for sel in children {
                write!(buf, "{},", sel).unwrap();
            }
            write!(buf, ")").unwrap();
        }
        self
    }
}

impl<Field: ResourceField> RequestBuilderTransformer for ObjectOption<Field> {
    fn trans(&self, mut req: RequestBuilder) -> RequestBuilder {
        req = self.access_opt.trans(req);
        if let Some(s) = self.select_buf.get(1..) {
            req = req.query(&[("$select", s)]);
        }
        if let Some(s) = self.expand_buf.get(1..) {
            req = req.query(&[("$expand", s)]);
        }
        req
    }
}

impl<Field: ResourceField> Default for ObjectOption<Field> {
    fn default() -> Self {
        Self::new()
    }
}

/// Option for GET-like requests for a collection of resource objects.
#[derive(Debug)]
pub struct CollectionOption<Field> {
    obj_option: ObjectOption<Field>,
    order_buf: Option<String>,
    page_size_buf: Option<String>,
    get_count_buf: Option<bool>,
}

impl<Field: ResourceField> CollectionOption<Field> {
    /// Create an empty (default) option.
    pub fn new() -> Self {
        Self {
            obj_option: Default::default(),
            order_buf: None,
            page_size_buf: None,
            get_count_buf: None,
        }
    }

    /// Only response if the object matches the `tag`.
    ///
    /// # See also
    /// [`ObjectOption::if_match`][if_match]
    ///
    /// [if_match]: ./struct.ObjectOption.html#method.if_match
    pub fn if_match(mut self, tag: &Tag) -> Self {
        self.obj_option = self.obj_option.if_match(tag);
        self
    }

    /// Only response if the object does not match the `tag`.
    ///
    /// # See also
    /// [`ObjectOption::if_none_match`][if_none_match]
    ///
    /// [if_none_match]: ./struct.ObjectOption.html#method.if_none_match
    pub fn if_none_match(mut self, tag: &Tag) -> Self {
        self.obj_option = self.obj_option.if_none_match(tag);
        self
    }

    /// Select only some fields of the resource object.
    ///
    /// # See also
    /// [`ObjectOption::select`][select]
    ///
    /// [select]: ./struct.ObjectOption.html#method.select
    pub fn select(mut self, fields: &[Field]) -> Self {
        self.obj_option = self.obj_option.select(fields);
        self
    }

    /// Expand a field of the resource object.
    ///
    /// # See also
    /// [`ObjectOption::expand`][expand]
    ///
    /// [expand]: ./struct.ObjectOption.html#method.expand
    pub fn expand(mut self, field: Field, select_children: Option<&[&str]>) -> Self {
        self.obj_option = self.obj_option.expand(field, select_children);
        self
    }

    /// Specify the sort order of the items responsed.
    ///
    /// # Note
    /// If called more than once, only the last call make sense.
    ///
    /// # See also
    /// [Microsoft Docs](https://docs.microsoft.com/en-us/graph/query-parameters#orderby-parameter)
    pub fn order_by(mut self, field: Field, order: Order) -> Self {
        let order = match order {
            Order::Ascending => "asc",
            Order::Descending => "desc",
        };
        self.order_buf = Some(format!("{} {}", field.api_field_name(), order));
        self
    }

    /// Specify the number of items per page.
    ///
    /// # Note
    /// If called more than once, only the last call make sense.
    ///
    /// # See also
    /// [Microsoft Docs](https://docs.microsoft.com/en-us/graph/query-parameters#top-parameter)
    pub fn page_size(mut self, size: usize) -> Self {
        self.page_size_buf = Some(size.to_string());
        self
    }

    /// Specify to get the number of all items.
    ///
    /// # Note
    /// If called more than once, only the last call make sense.
    ///
    /// Set it when calling unsupported API will cause HTTP 400 Client Error.
    ///
    /// # See also
    /// [Microsoft Docs](https://docs.microsoft.com/en-us/graph/query-parameters#count-parameter)
    pub fn get_count(mut self, get_count: bool) -> Self {
        self.get_count_buf = Some(get_count);
        self
    }
}

impl<Field: ResourceField> RequestBuilderTransformer for CollectionOption<Field> {
    fn trans(&self, mut req: RequestBuilder) -> RequestBuilder {
        req = self.obj_option.trans(req);
        if let Some(s) = &self.order_buf {
            req = req.query(&[("$orderby", s)]);
        }
        if let Some(s) = &self.page_size_buf {
            req = req.query(&[("$top", s)]);
        }
        if let Some(v) = self.get_count_buf {
            let v = if v { "true" } else { "false" };
            req = req.query(&[("$count", v)]);
        }
        req
    }
}

impl<Field: ResourceField> Default for CollectionOption<Field> {
    fn default() -> Self {
        Self::new()
    }
}

/// Specify the sorting order.
///
/// Used in [`CollectionOption::order_by`][order_by].
///
/// [order_by]: ./struct.CollectionOption.html#method.order_by
#[derive(Debug, PartialEq, Eq)]
pub enum Order {
    /// Ascending order.
    Ascending,
    /// Descending order.
    Descending,
}

/// Option for PUT-like requests of `DriveItem`.
#[derive(Debug, Default)]
pub struct DriveItemPutOption {
    access_opt: AccessOption,
    conflict_behavior: Option<ConflictBehavior>,
}

impl DriveItemPutOption {
    /// Create an empty (default) option.
    pub fn new() -> Self {
        Default::default()
    }

    /// Only response if the object matches the `tag`.
    ///
    /// # See also
    /// [`ObjectOption::if_match`][if_match]
    ///
    /// [if_match]: ./struct.ObjectOption.html#method.if_match
    pub fn if_match(mut self, tag: &Tag) -> Self {
        self.access_opt = self.access_opt.if_match(tag);
        self
    }

    // `if_none_match` is not supported in PUT-like requests.

    /// Specify the behavior if the target item already exists.
    ///
    /// # Note
    /// This not only available for DELETE-like requests. Read the docs first.
    ///
    /// # See also
    /// `@microsoft.graph.conflictBehavior` of DriveItem on [Microsoft Docs](https://docs.microsoft.com/en-us/graph/api/resources/driveitem?view=graph-rest-1.0#instance-attributes)
    pub fn conflict_behavior(mut self, conflict_behavior: ConflictBehavior) -> Self {
        self.conflict_behavior = Some(conflict_behavior);
        self
    }

    pub(crate) fn get_conflict_behavior(&self) -> Option<ConflictBehavior> {
        self.conflict_behavior
    }
}

impl RequestBuilderTransformer for DriveItemPutOption {
    fn trans(&self, req: RequestBuilder) -> RequestBuilder {
        self.access_opt.trans(req)
    }
}
