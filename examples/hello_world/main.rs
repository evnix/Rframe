extern crate rustful;
use rustful::{Server, Router, Request, Response};

fn say_hello(request: &Request, response: &mut Response) {
	let person = match request.variables.find(&~"person") {
		Some(name) => name.to_str(),
		None => ~"stranger"
	};

	match response.write(format!("Hello, {}!", person).as_bytes()) {
		Err(e) => println!("error while writing hello: {}", e),
		_ => {}
	}
}

fn main() {
	let routes = ~[
		("/", say_hello),
		("/:person", say_hello)
	];

	let server = Server {
		router: ~Router::from_vec(routes),
		port: 8080
	};

	server.run();
}