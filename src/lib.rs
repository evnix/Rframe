//!A light HTTP framework with REST-like features. The main purpose of Rustful
//!is to create a simple, modular and non-intrusive foundation for HTTP
//!applications. It has a mainly stateless structure, which naturally allows
//!it to run both as one single server and as multiple instances in a cluster.
//!
//!A new server is created using the [`Server`][server] type, which contains
//!all the necessary settings as fields:
//!
//!```no_run
//!#[macro_use]
//!extern crate rustful;
//!use rustful::{Server, Handler, Context, Response, TreeRouter};
//!
//!struct Greeting(&'static str);
//!
//!impl Handler for Greeting {
//!    fn handle_request(&self, context: Context, response: Response) {
//!        //Check if the client accessed /hello/:name or /good_bye/:name
//!        if let Some(name) = context.variables.get("name") {
//!            //Use the value of :name
//!            response.send(format!("{}, {}", self.0, name));
//!        } else {
//!            response.send(self.0)
//!        }
//!    }
//!}
//!
//!# fn main() {
//!let my_router = insert_routes!{
//!    //Create a new TreeRouter
//!    TreeRouter::new() => {
//!        //Receive GET requests to /hello and /hello/:name
//!        "hello" => {
//!            Get: Greeting("hello"),
//!            ":name" => Get: Greeting("hello")
//!        },
//!        //Receive GET requests to /good_bye and /good_bye/:name
//!        "good_bye" => {
//!            Get: Greeting("good bye"),
//!            ":name" => Get: Greeting("good bye")
//!        }
//!    }
//!};
//!
//!Server {
//!    //Use a closure to handle requests.
//!    handlers: my_router,
//!    //Set the listening port to `8080`.
//!    host: 8080.into(),
//!    //Fill out everything else with default values.
//!    ..Server::default()
//!}.run();
//!# }
//!```
//!
//![server]: server/struct.Server.html

#![crate_name = "rustful"]

#![crate_type = "rlib"]

#![doc(html_root_url = "http://ogeon.github.io/docs/rustful/master/")]

#![cfg_attr(all(test, feature = "benchmark"), feature(test))]
#![cfg_attr(feature = "strict", deny(missing_docs))]
#![cfg_attr(feature = "strict", deny(warnings))]

#[cfg(test)]
#[cfg(feature = "benchmark")]
extern crate test;

#[cfg(test)]
extern crate tempdir;

#[cfg(feature = "rustc-serialize")]
extern crate rustc_serialize;

#[cfg(feature = "multipart")]
extern crate multipart;

extern crate url;
extern crate time;
extern crate hyper;
extern crate anymap;
extern crate phf;

pub use hyper::mime;
pub use hyper::method::Method;
pub use hyper::status::StatusCode;
pub use hyper::header;
pub use hyper::Result as HttpResult;
pub use hyper::Error as HttpError;
pub use hyper::version::HttpVersion;

pub use self::server::Server;
pub use self::context::Context;
pub use self::response::Response;
pub use self::response::Error;
pub use self::handler::Handler;
pub use self::router::Router;
pub use self::log::Log;
pub use self::router::TreeRouter;

mod utils;
#[macro_use]
#[doc(hidden)]
pub mod macros;

pub mod server;
pub mod router;
pub mod handler;
pub mod context;
pub mod response;
pub mod filter;
pub mod log;
pub mod file;

use std::net::{SocketAddr, SocketAddrV4, SocketAddrV6, Ipv4Addr};
use std::str::FromStr;
use std::any::TypeId;
use std::collections::HashMap;
use std::ops::{Deref, DerefMut};
use std::borrow::Borrow;
use std::hash::Hash;
use std::fmt;

use anymap::Map;
use anymap::any::{Any, UncheckedAnyExt};

///HTTP or HTTPS.
pub enum Scheme {
    ///Standard HTTP.
    Http,

    ///HTTP with SSL encryption.
    #[cfg(feature = "ssl")]
    Https {
        ///Path to SSL certificate.
        cert: std::path::PathBuf,

        ///Path to key file.
        key: std::path::PathBuf
    }
}

///A host address and a port.
///
///Can be conveniently converted from an existing address-port pair or just a port:
///
///```
///use std::net::Ipv4Addr;
///use rustful::Host;
///
///let host1: Host = (Ipv4Addr::new(0, 0, 0, 0), 80).into();
///let host2: Host = 80.into();
///
///assert_eq!(host1, host2);
///```
#[derive(Eq, PartialEq, Debug, Hash, Clone, Copy)]
pub struct Host(SocketAddr);

impl Host {
    ///Create a `Host` with the address `0.0.0.0:port`. This is the same as `port.into()`.
    pub fn any_v4(port: u16) -> Host {
        Host(SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(0, 0, 0, 0), port)))
    }

    ///Change the port of the host address.
    pub fn port(&mut self, port: u16) {
        self.0 = match self.0 {
            SocketAddr::V4(addr) => SocketAddr::V4(SocketAddrV4::new(addr.ip().clone(), port)),
            SocketAddr::V6(addr) => {
                SocketAddr::V6(SocketAddrV6::new(addr.ip().clone(), port, addr.flowinfo(), addr.scope_id()))
            }
        };
    }
}

impl From<Host> for SocketAddr {
    fn from(host: Host) -> SocketAddr {
        host.0
    }
}

impl From<u16> for Host {
    fn from(port: u16) -> Host {
        Host::any_v4(port)
    }
}

impl From<SocketAddr> for Host {
    fn from(addr: SocketAddr) -> Host {
        Host(addr)
    }
}

impl From<SocketAddrV4> for Host {
    fn from(addr: SocketAddrV4) -> Host {
        Host(SocketAddr::V4(addr))
    }
}

impl From<SocketAddrV6> for Host {
    fn from(addr: SocketAddrV6) -> Host {
        Host(SocketAddr::V6(addr))
    }
}

impl From<(Ipv4Addr, u16)> for Host {
    fn from((ip, port): (Ipv4Addr, u16)) -> Host {
        Host(SocketAddr::V4(SocketAddrV4::new(ip, port)))
    }
}

impl FromStr for Host {
    type Err = <SocketAddr as FromStr>::Err;

    fn from_str(s: &str) -> Result<Host, Self::Err> {
        s.parse().map(|s| Host(s))
    }
}

///A somewhat lazy container for globally accessible data.
///
///It will try to be as simple as possible and allocate as little as possible,
///depending on the number of stored values.
///
/// * No value: Nothing is allocated and nothing is searched for during
///access.
///
/// * One value: One `Box` is allocated. Searching for a value will only
///consist of a comparison of `TypeId` and a downcast.
///
/// * Multiple values: An `AnyMap` is created, as well as a `Box` for each
///value. Searching for a value has the full overhead of `AnyMap`.
///
///`Global` can be created from a boxed value, from tuples or using the
///`Default` trait. More values can then be added using `insert(value)`.
///
///```
///# use rustful::Global;
///let mut g1: Global = Box::new(5).into();
///assert_eq!(g1.get(), Some(&5));
///assert_eq!(g1.get::<&str>(), None);
///
///let old = g1.insert(10);
///assert_eq!(old, Some(5));
///assert_eq!(g1.get(), Some(&10));
///
///g1.insert("cat");
///assert_eq!(g1.get(), Some(&10));
///assert_eq!(g1.get(), Some(&"cat"));
///
///let g2: Global = (5, "cat").into();
///assert_eq!(g2.get(), Some(&5));
///assert_eq!(g2.get(), Some(&"cat"));
///```
pub struct Global(GlobalState);

impl Global {
    ///Borrow a value of type `T` if the there is one.
    pub fn get<T: Any + Send + Sync>(&self) -> Option<&T> {
        match self.0 {
            GlobalState::None => None,
            GlobalState::One(id, ref a) => if id == TypeId::of::<T>() {
                //Here be dragons!
                unsafe { Some(a.downcast_ref_unchecked()) }
            } else {
                None
            },
            GlobalState::Many(ref map) => map.get()
        }
    }

    ///Insert a new value, returning the previous value of the same type, if
    ///any.
    pub fn insert<T: Any + Send + Sync>(&mut self, value: T) -> Option<T> {
        match self.0 {
            GlobalState::None => {
                *self = Box::new(value).into();
                None
            },
            GlobalState::One(id, _) => if id == TypeId::of::<T>() {
                if let GlobalState::One(_, ref mut previous_value) = self.0 {
                    let mut v = Box::new(value) as Box<Any + Send + Sync>;
                    std::mem::swap(previous_value, &mut v);
                    Some(unsafe { *v.downcast_unchecked() })
                } else {
                    unreachable!()
                }
            } else {
                //Here be more dragons!
                let mut other = GlobalState::Many(Map::new());
                std::mem::swap(&mut self.0, &mut other);
                if let GlobalState::Many(ref mut map) = self.0 {
                    if let GlobalState::One(id, previous_value) = other {
                        let mut raw = map.as_mut();
                        unsafe { raw.insert(id, previous_value); }
                    }

                    map.insert(value)
                } else {
                    unreachable!()
                }
            },
            GlobalState::Many(ref mut map) => {
                map.insert(value)
            }
        }
    }
}

impl<T: Any + Send + Sync> From<Box<T>> for Global {
    fn from(data: Box<T>) -> Global {
        Global(GlobalState::One(TypeId::of::<T>(), data))
    }
}

macro_rules! from_tuple {
    ($first: ident, $($t: ident),+) => (
        impl<$first: Any + Send + Sync, $($t: Any + Send + Sync),+> From<($first, $($t),+)> for Global {
            #[allow(non_snake_case)]
            fn from(tuple: ($first, $($t),+))-> Global {
                let ($first, $($t),+) = tuple;
                let mut map = Map::new();
                map.insert($first);
                $(
                    map.insert($t);
                )+

                Global(GlobalState::Many(map))
            }
        }

        from_tuple!($($t),+);
    );
    ($ty: ident) => (
        impl<$ty: Any + Send + Sync> From<($ty,)> for Global {
            fn from(tuple: ($ty,)) -> Global {
                Box::new(tuple.0).into()
            }
        }
    );
}

impl From<()> for Global {
    fn from(_: ()) -> Global {
        Global(GlobalState::None)
    }
}

from_tuple!(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11);

impl Default for Global {
    fn default() -> Global {
        Global(GlobalState::None)
    }
}

enum GlobalState {
    None,
    One(TypeId, Box<Any + Send + Sync>),
    Many(Map<Any + Send + Sync>),
}

///An extended `HashMap` with extra functionality for value partsing.
#[derive(Clone)]
pub struct Parameters<K, V>(HashMap<K, V>);

impl<K: Hash + Eq, V: AsRef<str>> Parameters<K, V> {
    ///Create an empty `Parameters`.
    pub fn new() -> Parameters<K, V> {
        Parameters(HashMap::new())
    }

    ///Try to parse an entry as `T`, if it exists. The error will be `None` if
    ///the entry does not exist, and `Some` if it does exists, but the parsing
    ///failed.
    ///
    ///```
    ///# use rustful::{Context, Response};
    ///fn my_handler(context: Context, response: Response) {
    ///    let age: Result<u8, _> = context.variables.parse("age");
    ///    match age {
    ///        Ok(age) => response.send(format!("age: {}", age)),
    ///        Err(Some(_)) => response.send("age must be a positive number"),
    ///        Err(None) => response.send("no age provided")
    ///    }
    ///}
    ///```
    pub fn parse<Q: ?Sized, T>(&self, key: &Q) -> Result<T, Option<T::Err>> where
        K: Borrow<Q>,
        Q: Hash + Eq,
        T: FromStr
    {
        if let Some(val) = self.0.get(key) {
            val.as_ref().parse().map_err(|e| Some(e))
        } else {
            Err(None)
        }
    }

    ///Try to parse an entry as `T`, if it exists, or return the default in
    ///`or`.
    ///
    ///```
    ///# use rustful::{Context, Response};
    ///fn my_handler(context: Context, response: Response) {
    ///    let page = context.variables.parse_or("page", 0u8);
    ///    response.send(format!("current page: {}", page));
    ///}
    ///```
    pub fn parse_or<Q: ?Sized, T>(&self, key: &Q, or: T) -> T where
        K: Borrow<Q>,
        Q: Hash + Eq,
        T: FromStr
    {
        self.parse(key).unwrap_or(or)
    }

    ///Try to parse an entry as `T`, if it exists, or create a new one using
    ///`or_else`. The `or_else` function will receive the parsing error if the
    ///value existed, but was impossible to parse.
    ///
    ///```
    ///# use rustful::{Context, Response};
    ///# fn do_heavy_stuff() -> u8 {0}
    ///fn my_handler(context: Context, response: Response) {
    ///    let science = context.variables.parse_or_else("science", |_| do_heavy_stuff());
    ///    response.send(format!("science value: {}", science));
    ///}
    ///```
    pub fn parse_or_else<Q: ?Sized, T, F>(&self, key: &Q, or_else: F) -> T where
        K: Borrow<Q>,
        Q: Hash + Eq,
        T: FromStr,
        F: FnOnce(Option<T::Err>) -> T
    {
        self.parse(key).unwrap_or_else(or_else)
    }
}

impl<K: Eq + Hash, V: AsRef<str>> Deref for Parameters<K, V> {
    type Target = HashMap<K, V>;

    fn deref(&self) -> &HashMap<K, V> {
        &self.0
    }
}

impl<K: Eq + Hash, V: AsRef<str>> DerefMut for Parameters<K, V> {
    fn deref_mut(&mut self) -> &mut HashMap<K, V> {
        &mut self.0
    }
}

impl<K: Eq + Hash, V: AsRef<str>> Into<HashMap<K, V>> for Parameters<K, V> {
    fn into(self) -> HashMap<K, V> {
        self.0
    }
}

impl<K: Eq + Hash, V: AsRef<str>> From<HashMap<K, V>> for Parameters<K, V> {
    fn from(map: HashMap<K, V>) -> Parameters<K, V> {
        Parameters(map)
    }
}

impl<K: Eq + Hash, V: AsRef<str> + PartialEq> PartialEq for Parameters<K, V> {
    fn eq(&self, other: &Parameters<K, V>) -> bool {
        self.0.eq(&other.0)
    }
}

impl<K: Eq + Hash, V: AsRef<str> + Eq> Eq for Parameters<K, V> {}

impl<K: Eq + Hash + fmt::Debug, V: AsRef<str> + fmt::Debug> fmt::Debug for Parameters<K, V> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl<K: Eq + Hash, V: AsRef<str>> Default for Parameters<K, V> {
    fn default() -> Parameters<K, V> {
        Parameters::new()
    }
}

impl<K: Eq + Hash, V: AsRef<str>> IntoIterator for Parameters<K, V> {
    type IntoIter = <HashMap<K, V> as IntoIterator>::IntoIter;
    type Item = (K, V);

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<'a, K: Eq + Hash, V: AsRef<str>> IntoIterator for &'a Parameters<K, V> {
    type IntoIter = <&'a HashMap<K, V> as IntoIterator>::IntoIter;
    type Item = (&'a K, &'a V);

    fn into_iter(self) -> Self::IntoIter {
        (&self.0).into_iter()
    }
}

impl<'a, K: Eq + Hash, V: AsRef<str>> IntoIterator for &'a mut Parameters<K, V> {
    type IntoIter = <&'a mut HashMap<K, V> as IntoIterator>::IntoIter;
    type Item = (&'a K, &'a mut V);

    fn into_iter(self) -> Self::IntoIter {
        (&mut self.0).into_iter()
    }
}

impl<'a, K: Eq + Hash, V: AsRef<str>> std::iter::FromIterator<(K, V)> for Parameters<K, V> {
    fn from_iter<T: IntoIterator<Item=(K, V)>>(iterable: T) -> Parameters<K, V> {
        HashMap::from_iter(iterable).into()
    }
}

impl<'a, K: Eq + Hash, V: AsRef<str>> Extend<(K, V)> for Parameters<K, V> {
    fn extend<T: IntoIterator<Item=(K, V)>>(&mut self, iter: T) {
        self.0.extend(iter)
    }
}
