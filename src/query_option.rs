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
use std::fmt::Write;

/// Option for a request to resource object.
#[derive(Debug, Default)]
pub struct ObjectOption {
    select_buf: String,
    expand_buf: String,
}

impl ObjectOption {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn select(mut self, fields: &[&str]) -> Self {
        for sel in fields {
            write!(self.select_buf, ",{}", sel).unwrap();
        }
        self
    }

    pub fn expand(mut self, field: &str, select_children: Option<&[&str]>) -> Self {
        let buf = &mut self.expand_buf;
        match select_children {
            None => write!(buf, ",{}", field).unwrap(),
            Some(children) => {
                write!(buf, ",{}($select=", field).unwrap();
                for sel in children {
                    write!(buf, "{},", sel).unwrap();
                }
                write!(buf, ")").unwrap();
            }
        }
        self
    }

    pub(crate) fn params(&self) -> impl Iterator<Item = (&str, &str)> {
        std::iter::empty()
            .chain(self.select_buf.get(1..).map(|s| ("$select", s)))
            .chain(self.expand_buf.get(1..).map(|s| ("$expand", s)))
    }
}

/// Option for the request to a collection of resource objects.
#[derive(Debug, Default)]
pub struct CollectionOption {
    q: ObjectOption,
    order_buf: Option<String>,
    page_size_buf: Option<String>,
    get_count_buf: Option<bool>,
}

impl CollectionOption {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn select(mut self, fields: &[&str]) -> Self {
        self.q = self.q.select(fields);
        self
    }

    pub fn expand(mut self, field: &str, select_children: Option<&[&str]>) -> Self {
        self.q = self.q.expand(field, select_children);
        self
    }

    pub fn order_by(mut self, field: &str, order: Order) -> Self {
        let order = match order {
            Order::Ascending => "asc",
            Order::Descending => "desc",
        };
        self.order_buf = Some(format!("{} {}", field, order));
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

#[derive(Debug, PartialEq, Eq)]
pub enum Order {
    Ascending,
    Descending,
}
