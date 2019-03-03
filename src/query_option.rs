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
//! https://docs.microsoft.com/en-us/graph/query-parameters
use crate::resource::{ResourceFieldOf, ResourceFieldTypeOf};
use std::default::Default;
use std::fmt::Write;
use std::marker::PhantomData;

/// Option for a request to resource object.
#[derive(Debug)]
pub struct ObjectOption<T> {
    select_buf: String,
    expand_buf: String,
    _marker: PhantomData<Fn(&T)>,
}

impl<T> ObjectOption<T> {
    pub fn new() -> Self {
        Self {
            select_buf: String::new(),
            expand_buf: String::new(),
            _marker: PhantomData,
        }
    }

    pub fn select(mut self, fields: &[&dyn ResourceFieldOf<T>]) -> Self {
        for sel in fields {
            write!(self.select_buf, ",{}", sel.api_field_name()).unwrap();
        }
        self
    }

    pub fn expand<Field: ResourceFieldTypeOf<T>>(
        mut self,
        field: Field,
        select_children: Option<&[&dyn ResourceFieldOf<Field>]>,
    ) -> Self {
        let buf = &mut self.expand_buf;
        write!(buf, ",{}", field.api_field_name()).unwrap();
        if let Some(children) = select_children {
            write!(buf, "($select=").unwrap();
            for sel in children {
                write!(buf, "{},", sel.api_field_name()).unwrap();
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

impl<T> Default for ObjectOption<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// Option for the request to a collection of resource objects.
#[derive(Debug)]
pub struct CollectionOption<T> {
    q: ObjectOption<T>,
    order_buf: Option<String>,
    page_size_buf: Option<String>,
    get_count_buf: Option<bool>,
}

impl<T> CollectionOption<T> {
    pub fn new() -> Self {
        Self {
            q: Default::default(),
            order_buf: None,
            page_size_buf: None,
            get_count_buf: None,
        }
    }

    pub fn select(mut self, fields: &[&dyn ResourceFieldOf<T>]) -> Self {
        self.q = self.q.select(fields);
        self
    }

    pub fn expand<Field: ResourceFieldTypeOf<T>>(
        mut self,
        field: Field,
        select_children: Option<&[&dyn ResourceFieldOf<Field>]>,
    ) -> Self {
        self.q = self.q.expand(field, select_children);
        self
    }

    pub fn order_by<Field: ResourceFieldOf<T>>(mut self, field: Field, order: Order) -> Self {
        let order = match order {
            Order::Ascending => "asc",
            Order::Descending => "desc",
        };
        self.order_buf = Some(format!("{} {}", field.api_field_name(), order));
        self
    }

    pub fn page_size(mut self, size: usize) -> Self {
        self.page_size_buf = Some(size.to_string());
        self
    }

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

impl<T> Default for CollectionOption<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum Order {
    Ascending,
    Descending,
}
