//!`Router` stores items, such as request handlers, using an HTTP method and a path as keys.
//!
//!```rust
//!# use rustful::router::Router;
//!# use http::method::Get;
//!# fn about_us() {}
//!# fn show_user() {}
//!# fn show_product() {}
//!# fn show_error() {}
//!# fn show_welcome() {}
//!let routes = [
//!	(Get, "/about", about_us),
//!	(Get, "/user/:user", show_user),
//!	(Get, "/product/:name", show_product),
//!	(Get, "/*", show_error),
//!	(Get, "/", show_welcome)
//!];
//!
//!let router = Router::from_routes(routes);
//!```
use collections::hashmap::HashMap;
use http::method::Method;
use std::vec::Vec;

pub type RouterResult<'a, T> = Option<(&'a T, ~HashMap<~str, ~str>)>;


///Stores items, such as request handlers, using an HTTP method and a path as keys.
///
///Paths can be static (`"path/to/item"`) or variable (`"users/:group/:user"`)
///and contain wildcards (`"path/*/item/*"`).
///
///Variables (starting with `:`) will match whatever word the request path contains at
///that point and it will be sent as a value to the item.
///
///Wildcards (a single `*`) will consume the segments until the rest of the
///path gives a match.
///
///```ignore
///pattern = "a/*/b"
///"a/c/b" -> match
///"a/c/d/b" -> match
///"a/b" -> no match
///"a/c/b/d" -> no match
///```
///
///```ignore
///pattern = "a/b/*"
///"a/b/c" -> match
///"a/b/c/d" -> match
///"a/b" -> no match
///```
#[deriving(Clone)]
pub struct Router<T> {
	items: HashMap<~str, (T, Vec<~str>)>,
	static_routes: HashMap<~str, ~Router<T>>,
	variable_route: Option<~Router<T>>,
	wildcard_route: Option<~Router<T>>
}

impl<T: Clone> Router<T> {
	///Creates an empty `Router`.
	pub fn new() -> Router<T> {
		Router {
			items: HashMap::new(),
			static_routes: HashMap::new(),
			variable_route: None,
			wildcard_route: None
		}
	}

	///Generates a `Router` tree from a set of items and paths.
	pub fn from_routes(routes: &[(Method, &str, T)]) -> Router<T> {
		let mut root = Router::new();

		for &(ref method, path, ref item) in routes.iter() {
			root.insert_item(method.clone(), path, item.clone());
		}

		root
	}

	///Inserts an item into the `Router` at a given path.
	pub fn insert_item(&mut self, method: Method, path: &str, item: T) {
		self.insert_item_vec(method, Router::<T>::path_to_vec(path.trim()).as_slice(), vec!(), item);
	}

	//Same as `insert_item`, but internal
	fn insert_item_vec(&mut self, method: Method, path: &[&str], variable_names: Vec<~str>, item: T) {
		let mut var_names = variable_names;

		match path {
			[piece] => {
				let next = self.find_or_insert_router(piece);
				if piece.len() > 0 && piece.char_at(0) == ':' {
					var_names.push(piece.slice(1, piece.len()).to_owned());
				}

				next.items.insert(method.to_str(), (item, var_names));
			},
			[piece, ..rest] => {
				let next = self.find_or_insert_router(piece);
				if piece.len() > 0 && piece.char_at(0) == ':' {
					var_names.push(piece.slice(1, piece.len()).to_owned());
				}
				next.insert_item_vec(method, rest, var_names, item);
			},
			[] => {
				self.items.insert(method.to_str(), (item, var_names));
			}
		}
	}

	///Insert an other Router at a path. The content of the other Router will be copied and merged with this one.
	pub fn insert_router(&mut self, path: &str, router: &~Router<T>) {
		self.insert_router_vec(Router::<T>::path_to_vec(path.trim()).as_slice(), vec!(), router);
	}

	//Same as `insert_router`, but internal
	fn insert_router_vec(&mut self, path: &[&str], variable_names: Vec<~str>, router: &~Router<T>) {
		let mut var_names = variable_names;

		match path {
			[piece] => {
				let next = self.find_or_insert_router(piece);
				if piece.len() > 0 && piece.char_at(0) == ':' {
					var_names.push(piece.slice(1, piece.len()).to_owned());
				}
				next.merge_router(var_names, router);
			},
			[piece, ..rest] => {
				let next = self.find_or_insert_router(piece);
				if piece.len() > 0 && piece.char_at(0) == ':' {
					var_names.push(piece.slice(1, piece.len()).to_owned());
				}
				next.insert_router_vec(rest, var_names, router);
			},
			[] => {
				self.merge_router(var_names, router);
			}
		}
	}

	//Mergers this Router with an other Router.
	fn merge_router(&mut self, variable_names: Vec<~str>, router: &~Router<T>) {
		for (key, &(ref item, ref var_names)) in router.items.iter() {
			let mut new_var_names = variable_names.clone();
			new_var_names.push_all(var_names.as_slice());
			self.items.insert(key.clone(), (item.clone(), new_var_names));
		}

		for (key, router) in router.static_routes.iter() {
			let next = self.static_routes.find_or_insert(key.clone(), ~Router::new());
			next.merge_router(variable_names.clone(), router);
		}

		if router.variable_route.is_some() {
			if self.variable_route.is_none() {
				self.variable_route = Some(~Router::new());
			}

			self.variable_route.as_mut().mutate(|next| {
				next.merge_router(variable_names.clone(), router.variable_route.as_ref().unwrap());
				next
			});
			
		}

		if router.wildcard_route.is_some() {
			if self.wildcard_route.is_none() {
				self.wildcard_route = Some(~Router::new());
			}

			self.wildcard_route.as_mut().mutate(|next| {
				next.merge_router(variable_names.clone(), router.wildcard_route.as_ref().unwrap());
				next
			});
		}
	}

	//Tries to find a router matching the key or inserts a new one if none exists
	fn find_or_insert_router<'a>(&'a mut self, key: &str) -> &'a mut ~Router<T> {
		if key == "*" {
			if self.wildcard_route.is_none() {
				self.wildcard_route = Some(~Router::new());
			}
			self.wildcard_route.as_mut::<'a>().unwrap()
		} else if key.len() > 0 && key.char_at(0) == ':' {
			if self.variable_route.is_none() {
				self.variable_route = Some(~Router::new());
			}
			self.variable_route.as_mut::<'a>().unwrap()
		} else {
			self.static_routes.find_or_insert_with::<'a>(key.to_owned(), |_| {
				~Router::new()
			})
		}
	}

	///Finds and returns the matching item and variables
	pub fn find<'a>(&'a self, method: Method, path: &str) -> RouterResult<'a, T> {
		self.search::<'a>(method, Router::<T>::path_to_vec(path.to_owned()).as_slice(), &[])
	}

	//Tries to find a matching item and run it
	fn search<'a>(&'a self, method: Method, path: &[&str], variables: &[&str]) -> RouterResult<'a, T> {
		match path {
			[piece] => {
				self.match_static(piece, &[], variables, |next, _, vars| { next.get::<'a>(method.clone(), vars) })
			},
			[piece, ..rest] => {
				self.match_static(piece, rest, variables, |next, path, vars| { next.search::<'a>(method.clone(), path, vars) })
			},
			[] => {
				self.get::<'a>(method, variables)
			}
		}
	}

	//Returns an item and a hashmap of variable values
	fn get<'a>(&'a self, method: Method, variables: &[&str]) -> RouterResult<'a, T> {
		match self.items.find(&method.to_str()) {
			Some(&(ref item, ref variable_names)) => {
				let mut var_map = ~HashMap::new();
				for (key, &value) in variable_names.iter().zip(variables.iter()) {
					var_map.insert(key.to_owned(), value.to_owned());
				}
				Some((item, var_map))
			},
			None => None
		}
	}

	//Checks for a static route. Runs `action` if found, runs `match_variable` otherwhise
	fn match_static<'a>(&'a self, key: &str, rest: &[&str], variables: &[&str], action: |&'a ~Router<T>, &[&str], &[&str]| -> RouterResult<'a, T>) -> RouterResult<'a, T> {
		match self.static_routes.find(&key.to_owned()) {
			Some(next) => {
				match action(next, rest, variables) {
					None => self.match_variable(key, rest, variables, action),
					result => result
				}
			},
			None => self.match_variable(key, rest, variables, action)
		}
	}

	//Checks for a variable route. Runs `action` if found, runs `match_wildcard` otherwhise
	fn match_variable<'a>(&'a self, key: &str, rest: &[&str], variables: &[&str], action: |&'a ~Router<T>, &[&str], &[&str]| -> RouterResult<'a, T>) -> RouterResult<'a, T> {
		match self.variable_route {
			Some(ref next) => {

				match action(next, rest, variables + &[key]) {
					None => self.match_wildcard(rest, variables, action),
					result => result
				}
			},
			None => self.match_wildcard(rest, variables, action)
		}
	}

	//Checks for a wildcard route. Runs `action` if found, returns `None` otherwhise
	fn match_wildcard<'a>(&'a self, rest: &[&str], variables: &[&str], action: |&'a ~Router<T>, &[&str], &[&str]| -> RouterResult<'a, T>) -> RouterResult<'a, T> {
		match self.wildcard_route {
			Some(ref next) => {
				let mut path = rest;
				while path.len() > 0 {
					match action(next, path, variables) {
						None => path = path.slice(1, path.len()),
						result => return result
					}
				}

				action(next, path, variables)
			},
			None => None
		}
	}

	//Converts a path to a suitable array of path segments
	fn path_to_vec<'a>(path: &'a str) -> Vec<&'a str> {
		if path.len() == 0 {
			vec!()
		} else if path.len() == 1 {
			if path == "/" {
				vec!()
			} else {
				vec!(path)
			}
		} else {
			let start = if path.char_at(0) == '/' { 1 } else { 0 };
			let end = if path.char_at(path.len() - 1) == '/' { 1 } else { 0 };
			path.slice(start, path.len() - end).split('/').collect::<Vec<&str>>()
		}
	}
}



#[cfg(test)]
mod test {
	use test::Bencher;
	use collections::hashmap::HashMap;
	use super::Router;
	use http::method::{Get, Post, Delete, Put, Head};
	use std::vec::Vec;

	fn check_variable(result: Option<(& &'static str, ~HashMap<~str, ~str>)>, expected: Option<&str>) {
		assert!(match result {
			Some((_, ref variables)) => match expected {
				Some(expected) => {
					let keys = vec!(~"a", ~"b", ~"c");
					let result = keys.iter().filter_map(|key| {
						match variables.find(key) {
							Some(value) => Some(value.to_owned()),
							None => None
						}
					}).collect::<Vec<~str>>().connect(", ");

					expected.to_str() == result
				},
				None => false
			},
			None => expected.is_none()
		});
	}

	fn check(result: Option<(& &'static str, ~HashMap<~str, ~str>)>, expected: Option<&str>) {
		assert!(match result {
			Some((result, _)) => match expected {
				Some(expected) => result.to_str() == expected.to_str(),
				None => false
			},
			None => expected.is_none()
		});
	}

	#[test]
	fn one_static_route() {
		let routes = [(Get, "path/to/test1", "test 1")];

		let router = Router::from_routes(routes);

		check(router.find(Get, "path/to/test1"), Some("test 1"));
		check(router.find(Get, "path/to"), None);
		check(router.find(Get, "path/to/test1/nothing"), None);
	}

	#[test]
	fn several_static_routes() {
		let routes = [
			(Get, "", "test 1"),
			(Get, "path/to/test/no2", "test 2"),
			(Get, "path/to/test1/no/test3", "test 3")
		];

		let router = Router::from_routes(routes);

		check(router.find(Get, ""), Some("test 1"));
		check(router.find(Get, "path/to/test/no2"), Some("test 2"));
		check(router.find(Get, "path/to/test1/no/test3"), Some("test 3"));
		check(router.find(Get, "path/to/test1/no"), None);
	}

	#[test]
	fn one_variable_route() {
		let routes = [(Get, "path/:a/test1", "test_var")];

		let router = Router::from_routes(routes);

		check_variable(router.find(Get, "path/to/test1"), Some("to"));
		check_variable(router.find(Get, "path/to"), None);
		check_variable(router.find(Get, "path/to/test1/nothing"), None);
	}

	#[test]
	fn several_variable_routes() {
		let routes = [
			(Get, "path/to/test1", "test_var"),
			(Get, "path/:a/test/no2", "test_var"),
			(Get, "path/to/:b/:c/:a", "test_var"),
			(Post, "path/to/:c/:a/:b", "test_var")
		];

		let router = Router::from_routes(routes);

		check_variable(router.find(Get, "path/to/test1"), Some(""));
		check_variable(router.find(Get, "path/to/test/no2"), Some("to"));
		check_variable(router.find(Get, "path/to/test1/no/test3"), Some("test3, test1, no"));
		check_variable(router.find(Post, "path/to/test1/no/test3"), Some("no, test3, test1"));
		check_variable(router.find(Get, "path/to/test1/no"), None);
	}

	#[test]
	fn one_wildcard_end_route() {
		let routes = [(Get, "path/to/*", "test 1")];

		let router = Router::from_routes(routes);

		check(router.find(Get, "path/to/test1"), Some("test 1"));
		check(router.find(Get, "path/to/same/test1"), Some("test 1"));
		check(router.find(Get, "path/to/the/same/test1"), Some("test 1"));
		check(router.find(Get, "path/to"), None);
		check(router.find(Get, "path"), None);
	}


	#[test]
	fn one_wildcard_middle_route() {
		let routes = [(Get, "path/*/test1", "test 1")];

		let router = Router::from_routes(routes);

		check(router.find(Get, "path/to/test1"), Some("test 1"));
		check(router.find(Get, "path/to/same/test1"), Some("test 1"));
		check(router.find(Get, "path/to/the/same/test1"), Some("test 1"));
		check(router.find(Get, "path/to"), None);
		check(router.find(Get, "path"), None);
	}

	#[test]
	fn one_universal_wildcard_route() {
		let routes = [(Get, "*", "test 1")];

		let router = Router::from_routes(routes);

		check(router.find(Get, "path/to/test1"), Some("test 1"));
		check(router.find(Get, "path/to/same/test1"), Some("test 1"));
		check(router.find(Get, "path/to/the/same/test1"), Some("test 1"));
		check(router.find(Get, "path/to"), Some("test 1"));
		check(router.find(Get, "path"), Some("test 1"));
		check(router.find(Get, ""), None);
	}

	#[test]
	fn several_wildcards_routes() {
		let routes = [
			(Get, "path/to/*", "test 1"),
			(Get, "path/*/test/no2", "test 2"),
			(Get, "path/to/*/*/*", "test 3")
		];

		let router = Router::from_routes(routes);

		check(router.find(Get, "path/to/test1"), Some("test 1"));
		check(router.find(Get, "path/for/test/no2"), Some("test 2"));
		check(router.find(Get, "path/to/test1/no/test3"), Some("test 3"));
		check(router.find(Get, "path/to/test1/no/test3/again"), Some("test 3"));
		check(router.find(Get, "path/to"), None);
	}

	#[test]
	fn route_formats() {
		let routes = [
			(Get, "/", "test 1"),
			(Get, "/path/to/test/no2", "test 2"),
			(Get, "path/to/test3/", "test 3"),
			(Get, "/path/to/test3/again/", "test 3")
		];

		let router = Router::from_routes(routes);

		check(router.find(Get, ""), Some("test 1"));
		check(router.find(Get, "path/to/test/no2/"), Some("test 2"));
		check(router.find(Get, "path/to/test3"), Some("test 3"));
		check(router.find(Get, "/path/to/test3/again"), Some("test 3"));
		check(router.find(Get, "//path/to/test3"), None);
	}

	#[test]
	fn http_methods() {
		let routes = [
			(Get, "/", "get"),
			(Post, "/", "post"),
			(Delete, "/", "delete"),
			(Put, "/", "put")
		];

		let router = Router::from_routes(routes);


		check(router.find(Get, "/"), Some("get"));
		check(router.find(Post, "/"), Some("post"));
		check(router.find(Delete, "/"), Some("delete"));
		check(router.find(Put, "/"), Some("put"));
		check(router.find(Head, "/"), None);
	}

	#[test]
	fn merge_routers() {
		let routes1 = [
			(Get, "", "test 1"),
			(Get, "path/to/test/no2", "test 2"),
			(Get, "path/to/test1/no/test3", "test 3")
		];

		let routes2 = [
			(Get, "", "test 1"),
			(Get, "test/no5", "test 5"),
			(Post, "test1/no/test3", "test 3 post")
		];

		let mut router1 = Router::from_routes(routes1);
		let router2 = ~Router::from_routes(routes2);

		router1.insert_router("path/to", &router2);

		check(router1.find(Get, ""), Some("test 1"));
		check(router1.find(Get, "path/to/test/no2"), Some("test 2"));
		check(router1.find(Get, "path/to/test1/no/test3"), Some("test 3"));
		check(router1.find(Post, "path/to/test1/no/test3"), Some("test 3 post"));
		check(router1.find(Get, "path/to/test/no5"), Some("test 5"));
		check(router1.find(Get, "path/to"), Some("test 1"));
	}


	#[test]
	fn merge_routers_variables() {
		let routes1 = [(Get, ":a/:b/:c", "test 2")];
		let routes2 = [(Get, ":b/:c/test", "test 1")];

		let mut router1 = Router::from_routes(routes1);
		let router2 = ~Router::from_routes(routes2);

		router1.insert_router(":a", &router2);
		
		check_variable(router1.find(Get, "path/to/test1"), Some("path, to, test1"));
		check_variable(router1.find(Get, "path/to/test1/test"), Some("path, to, test1"));
	}


	#[test]
	fn merge_routers_wildcard() {
		let routes1 = [(Get, "path/to", "test 2")];
		let routes2 = [(Get, "*/test1", "test 1")];

		let mut router1 = Router::from_routes(routes1);
		let router2 = ~Router::from_routes(routes2);

		router1.insert_router("path", &router2);

		check(router1.find(Get, "path/to/test1"), Some("test 1"));
		check(router1.find(Get, "path/to/same/test1"), Some("test 1"));
		check(router1.find(Get, "path/to/the/same/test1"), Some("test 1"));
		check(router1.find(Get, "path/to"), Some("test 2"));
		check(router1.find(Get, "path"), None);
	}

	
	#[bench]
	fn search_speed(b: &mut Bencher) {
		let routes = [
			(Get, "path/to/test1", "test 1"),
			(Get, "path/to/test/no2", "test 1"),
			(Get, "path/to/test1/no/test3", "test 1"),
			(Get, "path/to/other/test1", "test 1"),
			(Get, "path/to/test/no2/again", "test 1"),
			(Get, "other/path/to/test1/no/test3", "test 1"),
			(Get, "path/to/test1", "test 1"),
			(Get, "path/:a/test/no2", "test 1"),
			(Get, "path/to/:b/:c/:a", "test 1"),
			(Get, "path/to/*", "test 1"),
			(Get, "path/to/*/other", "test 1")
		];

		let paths = [
			"path/to/test1",
			"path/to/test/no2",
			"path/to/test1/no/test3",
			"path/to/other/test1",
			"path/to/test/no2/again",
			"other/path/to/test1/no/test3",
			"path/a/test1",
			"path/a/test/no2",
			"path/to/b/c/a",
			"path/to/test1/no",
			"path/to",
			"path/to/test1/nothing/at/all"
		];

		let router = Router::from_routes(routes);
		let mut counter = 0;

		b.iter(|| {
			router.find(Get, paths[counter].to_owned());
			counter = (counter + 1) % paths.len()
		});
	}

	
	#[bench]
	fn wildcard_speed(b: &mut Bencher) {
		let routes = [
			(Get, "*/to/*/*/a", "test 1")
		];

		let paths = [
			"path/to/a",
			"path/to/test/a",
			"path/to/test1/no/a",
			"path/to/other/a",
			"path/to/test/no2/a",
			"other/path/to/test1/no/a",
			"path/a/a",
			"path/a/test/a",
			"path/to/b/c/a",
			"path/to/test1/a",
			"path/a",
			"path/to/test1/nothing/at/all/and/all/and/all/and/a"
		];

		let router = Router::from_routes(routes);
		let mut counter = 0;

		b.iter(|| {
			router.find(Get, paths[counter].to_owned());
			counter = (counter + 1) % paths.len()
		});
	}
}