#![feature(phase)]
#[phase(plugin)]
extern crate rustful_macros;

extern crate rustful;
extern crate http;
use rustful::{Server, TreeRouter, Request, Response};
use http::method::Get;

fn say_hello(request: Request, response: &mut Response) {
	let person = match request.variables.get(&"person".into_string()) {
		Some(name) => name.as_slice(),
		None => "stranger"
	};

	try_send!(response, format!("Hello, {}!", person) while "showing hello");
}

fn main() {
	println!("Visit http://localhost:8080 or http://localhost:8080/Peter (if your name is Peter) to try this example.");

	let routes = routes!{"/" => Get: say_hello, "/:person" => Get: say_hello};

	Server::new().port(8080).handlers(TreeRouter::from_routes(&routes)).run();
}