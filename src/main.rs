extern crate hyper;
extern crate toml;
extern crate git2;
extern crate url;
extern crate rustc_serialize;

use std::env;
use std::io;
use std::io::Read;
use std::io::Write;
use std::fs::{File};
use git2::*;
use std::str;
use std::fmt;
use std::fs;
use std::collections::BTreeMap;
use rustc_serialize::hex::ToHex;
use std::path::Path;

use url::form_urlencoded;
use hyper::{Client, Url};
use hyper::header::Authorization;
use rustc_serialize::json;

macro_rules! otry {
	($expr:expr) => (match $expr {
		std::option::Option::Some(val) => val,
		std::option::Option::None => {
			panic!("NOOOO") //return std::result::Result::Err(std::convert::From::from(Error::from_str("Error! None given!")))
		}
	})
}

fn main() {
	match run() {
		Ok(val) =>(),
		Err(err) => println!("Error: {}", err),
	}
}

fn run() -> Result<(), Error> {
	let commit_identifier = env::args().nth(1).expect("No arg found");
	println!("Checking commit identifier: {}", commit_identifier);
	let settings = load_toml("settings.toml");
	let repo = open_repo(settings.get("repo")
		.expect("Could not find repo path setting")
		.as_str().unwrap());
	let diff = get_diff_for_commit(&repo, &commit_identifier).ok().expect("Diff loading failed");

	let filenames = get_changed_filenames(&diff);
	for st in filenames.ok().unwrap() {
		println!("Seen file change: {}; ", st);
	}

	let auth_token = get_token(&settings);
	let translate_to = settings.get("translate-to").unwrap().as_str().unwrap();
	let trans = MsTranslator { token: auth_token, lang_to: translate_to.to_string() };

	let subjects = try!(get_subjects_for_commit(&commit_identifier, &repo, &trans));

	//let answer_filename = format!("answers.{}.toml", commit_identifier);
	let answer_filename = "answers.toml";
	let exists = match fs::metadata(answer_filename) {
		Ok(val) => val.is_file(),
		Err(e) => false,
	};
	let mut answers = if exists {
		load_toml(answer_filename) } else { toml::Table::new() };
	conduct_asking(subjects, &mut answers);
	save_toml(answer_filename, answers);

	//ms_translate("Ich schreibe ein programm", translate_to, None, &auth_token);
	println!("Finished!");
	return Ok(());
}

fn open_repo(path: &str) -> Repository {
	return match Repository::open(path) {
		Ok(repo) => repo,
		Err(e) => panic!("failed to open git repo: {}", e),
	};
}

fn get_token(st: &toml::Table) -> AuthToken {
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
	let body_dec: AuthToken = json::decode(&body_res).unwrap();

	//println!("{}", body_res);
	return body_dec;
}

#[derive(RustcDecodable, RustcEncodable)]
struct AuthToken {
	token_type: String,
	access_token: String,
	expires_in: u64,
	scope: String,
}

struct MsTranslator {
	token: AuthToken,
	lang_to: String,
}

struct NoTranslator;

impl Translator for NoTranslator {
	fn translate(&self, text: &str, lang_from: Option<String>) -> String {
		return text.to_owned();
	}
	fn translate_s(&self, text: &str) -> String {
		return self.translate(text, None);
	}
}

impl Translator for MsTranslator {
	fn translate(&self, text: &str, lang_from: Option<String>) -> String {
		return ms_translate(text, self.lang_to.as_ref(), lang_from, &self.token);
	}
	fn translate_s(&self, text: &str) -> String {
		return self.translate(text, None);
	}
}

trait Translator {
	fn translate(&self, text: &str, lang_from: Option<String>) -> String;

	fn translate_s(&self, text: &str) -> String {
		return self.translate(text, None);
	}
}

fn ms_translate(text: &str, translate_to: &str, lang_from: Option<String>, at: &AuthToken) -> String {
	// documented at https://msdn.microsoft.com/en-us/library/ff512421.aspx
	let mut client = Client::new();
	let mut url = Url::parse("http://api.microsofttranslator.com/V2/Http.svc/Translate").unwrap();

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
		//.param("to", )
		//.param("text", &mut text).send().unwrap();
	let mut body = String::new();
	res.read_to_string(&mut body).unwrap();

	let mut body_stripped
		= &body[68 .. body.len() - 9]; //TODO better xml parsing
	println!("Translated {}", &body_stripped);

	return body_stripped.to_string();
}

fn load_toml(path: &str) -> toml::Table {
	let mut f = File::open(path)
		.ok()
		.expect(&format!("Failed to open toml file '{}'", path));
	let mut s = String::new();
	f.read_to_string(&mut s)
		.ok()
		.expect(&format!("Failed to read toml file '{}'", path));
	let mut parser = toml::Parser::new(&mut s);
	match parser.parse() {
		Some(value) => { /*println!("found toml: {:?}", value);*/ value },
		None => panic!("parse errors: {:?}", parser.errors),
	}
}

fn save_toml(path: &str, tbl: toml::Table) {
	let mut f = File::create(path)
		.ok()
		.expect(&format!("Failed to open toml file '{}'", path));
	let mut s = String::new();
	f.write_all(toml::Value::Table(tbl).to_string().as_bytes())
		.ok()
		.expect(&format!("Failed to write toml file '{}'", path));
}

enum PDDesc {
	Ok,
	NotOk,
	NoValid,
	Later,
}

fn pddesc_from_i(i: i8) -> Option<PDDesc> {
	return Some(match i {
		0 => PDDesc::Ok,
		1 => PDDesc::NotOk,
		2 => PDDesc::NoValid,
		3 => PDDesc::Later,
		_ => return None,
	});
}

#[derive(Default)]
struct QuestionSubject {
	commit_id: String,
	from_filename: String,
	orig: String,
	old: String,
	new: String,
	oldtrans: String,
	newtrans: String,
}

fn askq(qs: &QuestionSubject) -> PDDesc {
	println!("Original: '{}'\n\nOld: {}\nNew: {}\n\nOld translated: {}\nNew translated: {}",
		qs.orig, qs.old, qs.new, qs.oldtrans, qs.newtrans);

	let mut answ = String::from("Your answer: ");
	io::stdin().read_line(&mut answ)
		.ok()
		.expect("Failed to read line");
	let mut resp = PDDesc::NoValid;
	for x in answ.chars() {
		resp = match x {
			'y' => PDDesc::Ok,
			'n' => PDDesc::NotOk,
			'l' => PDDesc::Later,
			_ => resp,
		};
	}
	return resp;
}

impl QuestionSubject {
	fn get_subject_id(&self) -> String {
		return format!("{}:{}:{}", self.commit_id, self.from_filename, self.orig);
	}
}

/// Loads the question file,
fn conduct_asking(qsl: Vec<QuestionSubject>, answ: &mut toml::Table) {
	for qu in qsl {
		let subj_id = qu.get_subject_id();
		if !answ.contains_key(&subj_id) {
			match askq(&qu) {
				PDDesc::Ok=>{ answ.insert(subj_id,toml::Value::Boolean(true)); },
				PDDesc::NotOk=>{ answ.insert(subj_id,toml::Value::Boolean(false));},
				PDDesc::NoValid=>(), //Ignore :)
				PDDesc::Later=>(), //Okay then :)
			}
		} else {
			println!("Already reviewed string {}", subj_id);
			// already contained in ans!
		}
	}
}

// Git stuff

/// main parser handler and entry function
fn get_subjects_for_commit(commit_id: &str, repo: &Repository, trans: &Translator) -> Result<Vec<QuestionSubject>, Error> {
	let commit = try!(repo.find_commit(try!(Oid::from_str(commit_id))));
	let diff = try!(get_diff_for_commit(repo, commit_id));
	let old_tree = try!(try!(commit.parent(0)).tree());
	let new_tree = try!(commit.tree());

	return get_subjects_from_diff_and_trees(&diff, repo, old_tree, new_tree, trans, commit_id);
}

fn selfcontained_blob_parser(rep: &Repository, tree: &Tree, fname: &str, opt_btm: Option<&BTreeMap<String, String>>) -> Result<BTreeMap<String, String>, Error> {
	let obj = try!(get_obj_for_filename_and_tree(rep, tree, fname));
	let blob_cont = otry!(obj.as_blob()).content();
	return blob_parser(otry!(str::from_utf8(blob_cont).ok()), opt_btm);
}

fn blob_parser(blob_cont: &str, opt_btm: Option<&BTreeMap<String, String>>) -> Result<BTreeMap<String, String>, Error> {
	let mut res = BTreeMap::new();
	let mut msgid = None;
	for line in blob_cont.lines() {
		//local tlin = String::from(line).trim();
		if line.starts_with("msg") {
			if line.starts_with("msgid ") {
				match Some(&line["msgid ".len() .. ]) {
					Some(val) => msgid = Some(String::from(val.trim_matches('"'))),
					None =>(),
				}
			} else if line.starts_with("msgstr ") {
				match Some(&line["msgstr ".len() .. ]) {
					Some(val) => {
						let msg_raw_str = String::from(val.trim_matches('"'));
						match msgid {
							Some(msg_raw_id) => {
								if match opt_btm {
									Some(opt_btm_tr) => match opt_btm_tr.get(&msg_raw_id) {
											Some(old_msg_raw_str) => {/*println!("line {} exists and changed={}", line, old_msg_raw_str);*/ (&msg_raw_str != old_msg_raw_str)}, // record changed entries
											None => {/*println!("line {} is new", line);*/ true }, // record new entries
										},
									None => true, // record everything for the first run
								} {
									res.insert(msg_raw_id, msg_raw_str);
								}
							},
							None =>(), // TODO do sth, this is invalid format!!
						}
						msgid = None;
					},
					None =>(),
				}
			}
		};
	}
	return Ok(res);
}

fn get_obj_for_filename_and_tree<'repo>(rep: &'repo Repository, tree: &Tree, fname: &str) -> Result<Object<'repo>, Error> {
	let tree_entry = try!(tree.get_path(Path::new(fname))/*.ok_or(
		Error::from_str(&format!("file {} not found in tree {}",  fname, tree.id().as_bytes().to_hex())))*/);
	let tree_obj = try!(tree_entry.to_object(rep));
	return Ok(tree_obj);
}

fn get_subjects_from_diff_and_trees(diff: &Diff, repo: &Repository, tree_old: Tree, tree_new: Tree, trans: &Translator, commit_id: &str) -> Result<Vec<QuestionSubject>, Error> {
	let mut res = Vec::new();
	let changed_filenames = try!(get_changed_filenames(diff));
	for fname in changed_filenames {
		match fname.reason {
			FilenameChangeReason::add => {
				let fnamef = fname.filename.as_ref();
				let po_map = try!(selfcontained_blob_parser(repo, &tree_new, fnamef, None));
				// we have no old versions
				for (key, val) in po_map.iter() {
					res.push(QuestionSubject {
						commit_id: commit_id.to_string(),
						from_filename: fnamef.to_string(),
						orig: key.to_string(),
						old: "<no old version available>".to_string(),
						new: val.to_string(),
						oldtrans: "<no old version available>".to_string(),
						newtrans: trans.translate_s(val),
					});
				}
			},
			FilenameChangeReason::modify => {
				let fnamef = fname.filename.as_ref();
				let old_po_map = try!(selfcontained_blob_parser(repo, &tree_old, fnamef, None));
				let new_po_map = try!(selfcontained_blob_parser(repo, &tree_new, fnamef, Some(&old_po_map)));
				// we have old and new versions
				// the new po map is filled with the actually differing mentions
				for (key, val) in new_po_map.iter() {
					let oldval = old_po_map.get(key);
					res.push(QuestionSubject {
						commit_id: commit_id.to_string(),
						from_filename: fnamef.to_string(),
						orig: key.to_string(),
						old: match oldval { Some(v) => v.clone(), None => "?????".to_string()},
						new: val.to_string(),
						oldtrans: match oldval {
							Some(v) => trans.translate_s(v),
							None => "?????".to_string()},
						newtrans: trans.translate_s(val),
					});
				}
			},
			FilenameChangeReason::delete => {
				// do nothing here, perhaps notify...
			},
		}
	}
	return Ok(res);
}


enum FilenameChangeReason {
	add,
	modify,
	delete,
}

impl fmt::Display for FilenameChangeReason {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match *self {
			FilenameChangeReason::add => write!(f, "add"),
			FilenameChangeReason::modify => write!(f, "modify"),
			FilenameChangeReason::delete => write!(f, "delete"),
		}
	}
}

struct FilenameChange {
	reason: FilenameChangeReason,
	filename: String,
}


impl fmt::Display for FilenameChange {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "{} changed because {}", self.filename, self.reason)
	}
}

fn get_changed_filenames(diff: &Diff) -> Result<Vec<FilenameChange>, Error> {
	let mut res = Vec::new();
	try!(diff.print(DiffFormat::NameStatus, |_delta, _hunk, line| {
		let st = String::from(str::from_utf8(line.content()).unwrap());
		let mut stc = st.chars();
		let reason = match stc.next().unwrap() {
			'A' => FilenameChangeReason::add,
			'D' => FilenameChangeReason::delete,
			'M' => FilenameChangeReason::modify,
			_ => panic!("Unknown filename change!"),
		};
		let col: String = stc.collect();
		let fname = col.trim_left();
		res.push(FilenameChange{ reason: reason,
			filename: String::from(fname.trim_right())});
		true
	}));
	return Ok(res);
}

fn get_diff_for_commit(repo: &Repository, commit_id: &str) -> Result<Diff, Error> {
	let commit = try!(repo.find_commit(try!(Oid::from_str(commit_id))));
	let mut options = DiffOptions::new();
	return Diff::tree_to_tree(repo,
		try!(commit.parent(0)).tree().ok().as_ref(),
		commit.tree().ok().as_ref(),Some(&mut options));
}