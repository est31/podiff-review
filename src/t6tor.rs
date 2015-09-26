// podiff-review
// review po changes easily
//
// The MIT License (MIT)
//
// Copyright 2015 est31 <MTest31@outlook.com>
/*
Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in
all copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN
THE SOFTWARE.
*/

extern crate hyper;
extern crate toml;

use hyper::header::Authorization;
use hyper::{Client, Url};
use rustc_serialize::json;
use std::io::Read;
use url::form_urlencoded;

struct NoTranslator;

impl Translator for NoTranslator {
	fn translate(&self, text: &str, lang_from: Option<String>) -> String {
		return text.to_owned();
	}
	fn translate_s(&self, text: &str) -> String {
		return self.translate(text, None);
	}
}

pub trait Translator {
	fn translate(&self, text: &str, lang_from: Option<String>) -> String;

	fn translate_s(&self, text: &str) -> String {
		return self.translate(text, None);
	}
}

pub fn ms_translator(st: &toml::Table, lang_to: String) -> MsTranslator {
	return MsTranslator { token: ms_get_token(st), lang_to: lang_to };
}

#[derive(RustcDecodable, RustcEncodable)]
struct MsAuthToken {
	token_type: String,
	access_token: String,
	expires_in: u64,
	scope: String,
}

pub struct MsTranslator {
	token: MsAuthToken,
	lang_to: String,
}

impl Translator for MsTranslator {
	fn translate(&self, text: &str, lang_from: Option<String>) -> String {
		return ms_translate(text, self.lang_to.as_ref(), lang_from, &self.token);
	}
	fn translate_s(&self, text: &str) -> String {
		return self.translate(text, None);
	}
}

fn ms_translate(text: &str, translate_to: &str, lang_from: Option<String>, at: &MsAuthToken) -> String {
	// documented at https://msdn.microsoft.com/en-us/library/ff512421.aspx
	let mut client = Client::new();
	let mut url = Url::parse("http://api.microsofttranslator.com/V2/Http.svc/Translate").unwrap();

	// Fixes crash on translating empty strings, which are replied to by microsoft with
	// <string etc etc/> and not <string etc etc></string>,
	// which breaks our shit xml parsing
	if text.len() == 0 {
		return "".to_string();
	}

	match lang_from {
		Some(langc) => {
			url.set_query_from_pairs([
				("from", langc.as_ref())
			].iter().map(|&(k,v)| (k,v)));
		},
		None => (),
	}
	url.set_query_from_pairs([
			("to", translate_to),
			("text", text)
		].iter().map(|&(k,v)| (k,v)));
	//println!("URL:; {}", url.serialize());
	let mut res = client.get(url)
		.header(Authorization(format!(" Bearer {}", at.access_token)))
		.send().unwrap();
	let mut body = String::new();
	res.read_to_string(&mut body).unwrap();

	if body.len() < 68 + 9  {
		panic!(format!("Could not translate '{}': body has wrong format: '{}'", text, &body));
	}
	let mut body_stripped
		= &body[68 .. body.len() - 9]; //TODO better xml parsing
	println!("Translated {}", &body_stripped);

	return body_stripped.to_string();
}

fn ms_get_token(st: &toml::Table) -> MsAuthToken {
	// documented at https://msdn.microsoft.com/en-us/library/hh454950.aspx
	let mut client = Client::new();
	let client_id = st.get("ms-client-id").unwrap().as_str().unwrap();
	let client_secret = st.get("ms-auth-secret").unwrap().as_str().unwrap();
	let params = vec![
			("client_id", client_id),
			("client_secret", client_secret),
			("scope", "http://api.microsofttranslator.com"),
			("grant_type", "client_credentials")];
	let body = form_urlencoded::serialize(params.into_iter());

	// do the request
	let mut res = client.post("https://datamarket.accesscontrol.windows.net/v2/OAuth2-13")
		.body(&*body).send().unwrap();

	let mut body_res = String::new();
	res.read_to_string(&mut body_res).unwrap();
	let body_dec: MsAuthToken = json::decode(&body_res).unwrap();

	//println!("{}", body_res);
	return body_dec;
}
