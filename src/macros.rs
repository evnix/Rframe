#![crate_id = "rustful_macros#0.1-pre"]

#![comment = "Macros for rustful"]
#![license = "MIT"]
#![crate_type = "dylib"]

#![doc(html_root_url = "http://ogeon.github.io/rustful/doc/")]

#![feature(macro_rules, plugin_registrar, managed_boxes, quote, phase)]

//!This crate provides some helpful macros for rustful, including `router!` and `routes!`.
//!
//!#`router!`
//!The `router!` macro generates a `Router` containing the provided handlers and routing tree.
//!This can be useful to lower the risk of typing errors, among other things.
//!
//!##Example 1
//!
//!```rust
//!#![feature(phase)]
//!#[phase(plugin)]
//!extern crate rustful_macros;
//!
//!extern crate rustful;
//!
//!...
//!
//!
//!let router = router!{
//!	"/about" => Get: about_us,
//!	"/user/:user" => Get: show_user,
//!	"/product/:name" => Get: show_product,
//!	"/*" => Get: show_error,
//!	"/" => Get: show_welcome
//!};
//!```
//!
//!##Example 2
//!
//!```rust
//!#![feature(phase)]
//!#[phase(plugin)]
//!extern crate rustful_macros;
//!
//!extern crate rustful;
//!
//!...
//!
//!
//!let router = router!{
//!	"/" => Get: show_home,
//!	"home" => Get: show_home,
//!	"user/:username" => {Get: show_user, Post: save_user},
//!	"product" => {
//!		Get: show_all_products,
//!
//!		"json" => Get: send_all_product_data
//!		":id" => {
//!			Get: show_product,
//!			Post | Delete: edit_product,
//!
//!			"json" => Get: send_product_data
//!		}
//!	}
//!};
//!```
//!
//!#`routes!`
//!
//!The `routes!` macro generates a vector of routes, which can be used to create a `Router`.
//!It has the same syntax as `routes!`.
//!
//!```rust
//!#![feature(phase)]
//!#[phase(plugin)]
//!extern crate rustful_macros;
//!
//!extern crate rustful;
//!
//!...
//!
//!
//!let routes = routes!{
//!	"/" => Get: show_home,
//!	"home" => Get: show_home,
//!	"user/:username" => {Get: show_user, Post: save_user},
//!	"product" => {
//!		Get: show_all_products,
//!
//!		"json" => Get: send_all_product_data
//!		":id" => {
//!			Get: show_product,
//!			Post | Delete: edit_product,
//!
//!			"json" => Get: send_product_data
//!		}
//!	}
//!};
//!
//!let router = Router::from_routes(routes);
//!```

extern crate syntax;
extern crate rustc;

use std::path::BytesContainer;

use syntax::{ast, codemap};
use syntax::ext::base::{
	ExtCtxt, MacResult, MacExpr,
	NormalTT, BasicMacroExpander
};
use syntax::ext::build::AstBuilder;
use syntax::ext::quote::rt::ExtParseUtils;
use syntax::parse;
use syntax::parse::token;
use syntax::parse::parser;
use syntax::parse::parser::Parser;
//use syntax::print::pprust;

use rustc::plugin::Registry;

#[plugin_registrar]
#[doc(hidden)]
pub fn macro_registrar(reg: &mut Registry) {
	let expander = box BasicMacroExpander{expander: expand_router, span: None};
	reg.register_syntax_extension(token::intern("router"), NormalTT(expander, None));

	let expander = box BasicMacroExpander{expander: expand_routes, span: None};
	reg.register_syntax_extension(token::intern("routes"), NormalTT(expander, None));
}

fn expand_router(cx: &mut ExtCtxt, sp: codemap::Span, tts: &[ast::TokenTree]) -> Box<MacResult> {
	let router_ident = cx.ident_of("router");
	let insert_method = cx.ident_of("insert_item");

	let mut calls: Vec<@ast::Stmt> = vec!(
		cx.stmt_let(sp, true, router_ident, quote_expr!(&cx, ::rustful::Router::new()))
	);

	for (path, method, handler) in parse_routes(cx, tts).move_iter() {
		let path_expr = cx.parse_expr(format!("\"{}\"", path));
		let method_expr = cx.expr_path(method);
		calls.push(cx.stmt_expr(
			cx.expr_method_call(sp, cx.expr_ident(sp, router_ident), insert_method, vec!(method_expr, path_expr, handler))
		));
	}

	let block = cx.expr_block(cx.block(sp, calls, Some(cx.expr_ident(sp, router_ident))));
	
	MacExpr::new(block)
}

fn expand_routes(cx: &mut ExtCtxt, sp: codemap::Span, tts: &[ast::TokenTree]) -> Box<MacResult> {
	let routes = parse_routes(cx, tts).move_iter().map(|(path, method, handler)| {
		let path_expr = cx.parse_expr(format!("\"{}\"", path));
		let method_expr = cx.expr_path(method);
		mk_tup(sp, vec!(method_expr, path_expr, handler))
	}).collect();

	MacExpr::new(cx.expr_vec(sp, routes))
}

fn parse_routes(cx: &mut ExtCtxt, tts: &[ast::TokenTree]) -> Vec<(String, ast::Path, @ast::Expr)> {

	let mut parser = parse::new_parser_from_tts(
		cx.parse_sess(), cx.cfg(), Vec::from_slice(tts)
	);

	parse_subroutes("", cx, &mut parser)
}

fn parse_subroutes(base: &str, cx: &mut ExtCtxt, parser: &mut Parser) -> Vec<(String, ast::Path, @ast::Expr)> {
	let mut routes = Vec::new();

	while !parser.eat(&token::EOF) {
		match parser.parse_optional_str() {
			Some((ref s, _)) => {
				if !parser.eat(&token::FAT_ARROW) {
					parser.expect(&token::FAT_ARROW);
					break;
				}

				let mut new_base = base.to_string();
				match s.container_as_str() {
					Some(s) => {
						new_base.push_str(s.trim_chars('/'));
						new_base.push_str("/");
					},
					None => cx.span_err(parser.span, "invalid path")
				}

				if parser.eat(&token::EOF) {
					cx.span_err(parser.span, "unexpected end of routing tree");
				}

				if parser.eat(&token::LBRACE) {
					let subroutes = parse_subroutes(new_base.as_slice(), cx, parser);
					routes.push_all(subroutes.as_slice());

					if parser.eat(&token::RBRACE) {
						if !parser.eat(&token::COMMA) {
							break;
						}
					} else {
						parser.expect_one_of([token::COMMA, token::RBRACE], []);
					}
				} else {
					for (method, handler) in parse_handler(parser).move_iter() {
						routes.push((new_base.clone(), method, handler))
					}

					if !parser.eat(&token::COMMA) {
						break;
					}
				}
			},
			None => {
				for (method, handler) in parse_handler(parser).move_iter() {
					routes.push((base.to_string(), method, handler))
				}

				if !parser.eat(&token::COMMA) {
					break;
				}
			}
		}
	}

	routes
}

fn parse_handler(parser: &mut Parser) -> Vec<(ast::Path, @ast::Expr)> {
	let mut methods = Vec::new();

	loop {
		methods.push(parser.parse_path(parser::NoTypesAllowed).path);

		if parser.eat(&token::COLON) {
			break;
		}

		if !parser.eat(&token::BINOP(token::OR)) {
			parser.expect_one_of([token::COLON, token::BINOP(token::OR)], []);
		}
	}

	let handler = parser.parse_expr();

	methods.move_iter().map(|m| (m, handler.clone())).collect()
}

fn mk_tup(sp: codemap::Span, content: Vec<@ast::Expr>) -> @ast::Expr {
	dummy_expr(sp, ast::ExprTup(content))
}

fn dummy_expr(sp: codemap::Span, e: ast::Expr_) -> @ast::Expr {
	@ast::Expr {
		id: ast::DUMMY_NODE_ID,
		node: e,
		span: sp,
	}
}


/**
A macro for assigning content types.

It takes a main type, a sub type and a parameter list. Instead of this:

```
response.headers.content_type = Some(MediaType {
	type_: String::from_str("text"),
	subtype: String::from_str("html"),
	parameters: vec!((String::from_str("charset"), String::from_str("UTF-8")))
});
```

it can be written like this:

```
response.headers.content_type = content_type!("text", "html", "charset": "UTF-8");
```

The `"charset": "UTF-8"` part defines the parameter list for the content type.
It may contain more than one parameter, or be omitted:

```
response.headers.content_type = content_type!("application", "octet-stream", "type": "image/gif", "padding": "4");
```

```
response.headers.content_type = content_type!("image", "png");
```
**/
#[macro_export]
macro_rules! content_type(
	($main_type:expr, $sub_type:expr) => ({
		Some(::http::headers::content_type::MediaType {
			type_: String::from_str($main_type),
			subtype: String::from_str($sub_type),
			parameters: Vec::new()
		})
	});

	($main_type:expr, $sub_type:expr, $($param:expr: $value:expr),+) => ({
		Some(::http::headers::content_type::MediaType {
			type_: String::from_str($main_type),
			subtype: String::from_str($sub_type),
			parameters: vec!( $( (String::from_str($param), String::from_str($value)) ),+ )
		})
	});
)