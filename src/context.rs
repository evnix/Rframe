//!Handler context and request body reading extensions.

#![stable]

use std::collections::HashMap;
use std::old_io::IoResult;
use std::ops::{Deref, DerefMut};

use hyper::server::request::Request;

use utils;

use Method;
use header::Headers;

///A container for things like request data and cache.
///
///A `Context` can be dereferenced to a `BodyReader`, allowing direct access to
///the underlying read methods.
pub struct Context<'r, 'c, Cache: 'c =()> {
    ///Headers from the HTTP request.
    pub headers: Headers,

    ///The HTTP method.
    pub method: Method,

    ///The requested path.
    pub path: String,

    ///Route variables.
    pub variables: HashMap<String, String>,

    ///Query variables from the path.
    pub query: HashMap<String, String>,

    ///The fragment part of the URL (after #), if provided.
    pub fragment: Option<String>,

    ///Resource cache.
    pub cache: &'c Cache,

    ///A reader for the request body.
    pub body_reader: BodyReader<'r>
}

impl<'r, 'c, C> Deref for Context<'r, 'c, C> {
    type Target = BodyReader<'r>;

    fn deref<'a>(&'a self) -> &'a BodyReader<'r> {
        &self.body_reader
    }
}

impl<'r, 'c, C> DerefMut for Context<'r, 'c, C> {
    fn deref_mut<'a>(&'a mut self) -> &'a mut BodyReader<'r> {
        &mut self.body_reader
    }
}

///A reader for a request body.
pub struct BodyReader<'r> {
    request: Request<'r>
}

impl<'r> BodyReader<'r> {
    pub fn from_request(request: Request<'r>) -> BodyReader<'r> {
        BodyReader {
            request: request
        }
    }
}

impl<'r> Reader for BodyReader<'r> {
    ///Read the request body.
    fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        self.request.read(buf)
    }
}

///`BodyReader` extension for reading and parsing a query string.
pub trait ExtQueryBody {
    fn read_query_body(&mut self) -> IoResult<HashMap<String, String>>;
}

impl<'r> ExtQueryBody for BodyReader<'r> {
    ///Read and parse the request body as a query string.
    ///The body will be decoded as UTF-8 and plain '+' characters will be replaced with spaces.
    #[inline]
    fn read_query_body(&mut self) -> IoResult<HashMap<String, String>> {
        Ok(utils::parse_parameters(&*try!(self.read_to_end())))
    }
}