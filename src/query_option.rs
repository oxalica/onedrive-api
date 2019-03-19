//! Query options which can be used to customize responses.
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
use crate::resource::ResourceField;
use std::default::Default;
use std::fmt::Write;
use std::marker::PhantomData;

/// Option for a request to resource object.
#[derive(Debug)]
pub struct ObjectOption<Field> {
    select_buf: String,
    expand_buf: String,
    _marker: PhantomData<Fn(&Field)>,
}

impl<Field: ResourceField> ObjectOption<Field> {
    /// Create an empty (default) option.
    pub fn new() -> Self {
        Self {
            select_buf: String::new(),
            expand_buf: String::new(),
            _marker: PhantomData,
        }
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

    pub(crate) fn params(&self) -> impl Iterator<Item = (&str, &str)> {
        std::iter::empty()
            .chain(self.select_buf.get(1..).map(|s| ("$select", s)))
            .chain(self.expand_buf.get(1..).map(|s| ("$expand", s)))
    }
}

impl<Field: ResourceField> Default for ObjectOption<Field> {
    fn default() -> Self {
        Self::new()
    }
}

/// Option for the request to a collection of resource objects.
#[derive(Debug)]
pub struct CollectionOption<Field> {
    q: ObjectOption<Field>,
    order_buf: Option<String>,
    page_size_buf: Option<String>,
    get_count_buf: Option<bool>,
}

impl<Field: ResourceField> CollectionOption<Field> {
    /// Create an empty (default) option.
    pub fn new() -> Self {
        Self {
            q: Default::default(),
            order_buf: None,
            page_size_buf: None,
            get_count_buf: None,
        }
    }

    /// Select only some fields of the resource object.
    ///
    /// # See also
    /// [`ObjectOption::select`][select]
    ///
    /// [select]: ./struct.ObjectOption.html#method.select
    pub fn select(mut self, fields: &[Field]) -> Self {
        self.q = self.q.select(fields);
        self
    }

    /// Expand a field of the resource object.
    ///
    /// # See also
    /// [`ObjectOption::expand`][expand]
    ///
    /// [expand]: ./struct.ObjectOption.html#method.expand
    pub fn expand(mut self, field: Field, select_children: Option<&[&str]>) -> Self {
        self.q = self.q.expand(field, select_children);
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

    pub(crate) fn params(&self) -> impl Iterator<Item = (&str, &str)> {
        self.q
            .params()
            .chain(self.order_buf.as_ref().map(|s| ("$orderby", &**s)))
            .chain(self.page_size_buf.as_ref().map(|s| ("$top", &**s)))
            .chain(
                self.get_count_buf
                    .map(|v| ("$count", if v { "true" } else { "false" })),
            )
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
