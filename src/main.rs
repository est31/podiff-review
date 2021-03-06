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
extern crate git2;
extern crate url;
extern crate regex;
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
use std::collections::btree_map::Entry;
use std::path::Path;
use std::ops::Deref;
use regex::Regex;

mod t6tor;
use t6tor::*;

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

	let translate_to = settings.get("translate-to").unwrap().as_str().unwrap();
	let api_name = settings.get("translate-api").unwrap().as_str().unwrap();
	let trans = match api_name.as_ref() {
		"ms" => Box::new(t6tor::ms_translator(&settings, translate_to.to_string())) as Box<Translator>,
		"yn" => Box::new(t6tor::yn_translator(&settings, translate_to.to_string())) as Box<Translator>,
		_ => panic!("invalid API specifier for translate-to"),
	};

	if let Some(attri) = trans.attribution_info() {
		println!("\n{}\n", attri);
	}
	let regex_opt = settings.get("filename-regex").map_or(None, |s| s.as_str());

	let subjects = try!(get_subjects_for_commit(&commit_identifier, &repo, trans.deref(), regex_opt));

	//let answer_filename = format!("answers.{}.toml", commit_identifier);
	let answer_filename = "answers.toml";
	let exists = match fs::metadata(answer_filename) {
		Ok(val) => val.is_file(),
		Err(e) => false,
	};
	let mut answers = if exists {
		load_toml(answer_filename) } else { toml::Table::new() };
	conduct_asking(subjects, &mut answers, true);
	save_toml(answer_filename, answers);

	println!("Finished!");
	return Ok(());
}

fn open_repo(path: &str) -> Repository {
	return match Repository::open(path) {
		Ok(repo) => repo,
		Err(e) => panic!("failed to open git repo: {}", e),
	};
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
		Some(value) => value,
		None => panic!("parse error: {:?}", parser.errors),
	}
}

fn save_toml(path: &str, tbl: toml::Table) {
	let mut f = File::create(path)
		.ok()
		.expect(&format!("Failed to open toml file '{}'", path));
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

#[derive(Default)]
struct QuestionSubject {
	commit_id: String,
	from_filename: String,
	orig: String,
	old: Option<String>,
	new: String,
	oldtrans: String,
	newtrans: String,
}

fn askq(qs: &QuestionSubject) -> PDDesc {
	let no_available_str = "<no old version available>".to_string();
	println!("Original: '{}'\n\nOld: {}\nNew: {}\n\nOld translated: {}\nNew translated: {}",
		qs.orig, match qs.old { Some(ref v)=>v, None=>&no_available_str },
		qs.new, qs.oldtrans, qs.newtrans);

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

fn is_obviously_equal(qs: &QuestionSubject) -> bool {
	if qs.newtrans.to_lowercase() == qs.orig.to_lowercase() {
		return true;
	}
	// false otherwise
	return false;
}

impl QuestionSubject {
	fn get_subject_id(&self) -> String {
		return format!("{}:{}:{}", self.commit_id, self.from_filename, self.orig);
	}
}

fn conduct_asking(qsl: Vec<QuestionSubject>, answ: &mut toml::Table, reask_non_ok: bool) {
	let mut ok_old_ctr = 0;
	let mut notok_old_ctr = 0;
	let mut ok_new_ctr = 0;
	let mut notok_new_ctr = 0;
	let mut ignored_ctr = 0;

	for qu in qsl {
		let subj_id = qu.get_subject_id();
		match answ.entry(subj_id.clone()) {
			Entry::Vacant(e) => if is_obviously_equal(&qu) {
				println!("Fast-forwarding string '{}' because equal according to translator (ID {}).", qu.orig, subj_id);
				e.insert(toml::Value::Boolean(true));
				ok_new_ctr += 1;
			} else {
				match askq(&qu) {
					PDDesc::Ok => {
						e.insert(toml::Value::Boolean(true));
						ok_new_ctr += 1;
					},
					PDDesc::NotOk => {
						e.insert(toml::Value::Boolean(false));
						notok_new_ctr += 1
					},
					PDDesc::NoValid=>(), //Ignore :)
					PDDesc::Later=>{ ignored_ctr += 1 }, //Okay then :)
				}
			},
			Entry::Occupied(mut e) => {
				println!("Already reviewed string '{}' (ID {}).", qu.orig, subj_id);
				// already contained in ans!
				let val: &toml::Value = &e.get().clone();
				if match val.as_bool() {Some(w) => w, None => false} {
					ok_old_ctr += 1;
				} else {
					if reask_non_ok {
						match askq(&qu) {
							PDDesc::Ok => {
								e.insert(toml::Value::Boolean(true));
								ok_new_ctr += 1;
							},
							PDDesc::NotOk => {
								notok_old_ctr += 1;
							},
							PDDesc::NoValid=>(), //Ignore :)
							PDDesc::Later=>{
								e.remove();
								ignored_ctr += 1;
							},
						}
					} else {
						notok_old_ctr += 1;
					}
				}
			}
		}
	}
	if notok_new_ctr + notok_old_ctr + ignored_ctr == 0 {
		println!("Review succeeded ({} times ok, of which {} new and {} loaded from file)",
			ok_new_ctr + ok_old_ctr, ok_new_ctr, ok_old_ctr);
	} else {
		println!("Review not succeeded ({} times not ok ({} new), {} times ok ({} new), {} ignores)",
			notok_new_ctr + notok_old_ctr, notok_new_ctr,
			ok_new_ctr + ok_old_ctr, ok_new_ctr,
			ignored_ctr);
	}
}

// Git stuff

/// main parser handler and entry function
fn get_subjects_for_commit(commit_id: &str, repo: &Repository, trans: &Translator, filename_regex :Option<&str>) -> Result<Vec<QuestionSubject>, Error> {
	let commit = try!(repo.find_commit(try!(Oid::from_str(commit_id))));
	let diff = try!(get_diff_for_commit(repo, commit_id));
	let old_tree = try!(try!(commit.parent(0)).tree());
	let new_tree = try!(commit.tree());

	return get_subjects_from_diff_and_trees(&diff, repo, old_tree, new_tree, trans, commit_id, filename_regex);
}

fn selfcontained_blob_parser(rep: &Repository, tree: &Tree, fname: &str, opt_btm: Option<&BTreeMap<String, String>>) -> Result<BTreeMap<String, String>, Error> {
	let obj = try!(get_obj_for_filename_and_tree(rep, tree, fname));
	let blob_cont = otry!(obj.as_blob()).content();
	return blob_parser(otry!(str::from_utf8(blob_cont).ok()), opt_btm);
}

fn blob_parser(blob_cont: &str, opt_btm: Option<&BTreeMap<String, String>>) -> Result<BTreeMap<String, String>, Error> {
	let mut res = BTreeMap::new();
	let mut msgid: Option<String> = None;
	let mut msgstr: Option<String> = None;
	let mut multi_line_mode = 0; // 0 = off, 1 = for msgid, 2 = for msgstr
	macro_rules! handle_pair {
		() => { {
			if msgid.is_some() && msgstr.is_some() {
				let mut msg_raw_id = msgid.take().unwrap();
				let mut msg_raw_str = msgstr.take().unwrap();
				msg_raw_id = msg_raw_id.replace("\\n", " | ");
				msg_raw_str = msg_raw_str.replace("\\n", " | ");
				if match opt_btm {
					Some(opt_btm_tr) => match opt_btm_tr.get(&msg_raw_id) {
						Some(old_msg_raw_str) => (&msg_raw_str != old_msg_raw_str), // record changed entries
						None => true, // record new entries
					},
					None => true, // record everything for the first run
				} {
					// Allow everything except where msgid is "". This is special.
					if msg_raw_id != "" {
						res.insert(String::from(msg_raw_id), msg_raw_str);
					}
				}
			}
		} }
	}
	for line in blob_cont.lines() {
		if line.starts_with("\"") && (multi_line_mode > 0) {
			match multi_line_mode {
				1 => {
					let s = msgid.unwrap();
					msgid = Some(String::from(s) + line.trim_matches('"'));
				},
				2 => {
					let s = msgstr.unwrap();
					msgstr = Some(String::from(s) + line.trim_matches('"'));
				},
				_ => {
					panic!("This shouldn't happen!");
				},
			}
		} else {
			multi_line_mode = 0;
			handle_pair!();
		}
		if line.starts_with("msg") {

			if line.starts_with("msgid ") {
				msgid = Some(String::from(line["msgid ".len() .. ].trim_matches('"')));
				multi_line_mode = 1;
			} else if line.starts_with("msgstr ") {
				msgstr = Some(String::from(line["msgstr ".len() .. ].trim_matches('"')));
				multi_line_mode = 2;
			}
		};
	}
	handle_pair!();
	return Ok(res);
}

fn get_obj_for_filename_and_tree<'repo>(rep: &'repo Repository, tree: &Tree, fname: &str) -> Result<Object<'repo>, Error> {
	let tree_entry = try!(tree.get_path(Path::new(fname))/*.ok_or(
		Error::from_str(&format!("file {} not found in tree {}",  fname, tree.id().as_bytes().to_hex())))*/);
	let tree_obj = try!(tree_entry.to_object(rep));
	return Ok(tree_obj);
}

fn filename_to_language<'a>(filename :&'a str, regex :&str) -> Option<&'a str> {
	let re = Regex::new(regex).unwrap();;
	return re.captures(filename).map_or(None, |cap| cap.at(1));
}

fn get_subjects_from_diff_and_trees(diff: &Diff, repo: &Repository, tree_old: Tree, tree_new: Tree, trans: &Translator, commit_id: &str, filename_regex :Option<&str>) -> Result<Vec<QuestionSubject>, Error> {
	let mut res = Vec::new();
	let changed_filenames = try!(get_changed_filenames(diff));
	for fname in changed_filenames {
		if filename_regex.is_none() && !fname.filename.ends_with(".po") {
			println!("Ignoring non-po ending file {}", fname.filename);
			continue;
		}
		let from_lang = filename_regex.map_or(None, |regex| filename_to_language(&fname.filename, regex));
		if from_lang.is_some() {
			println!("Detected language '{}' for file {}", from_lang.unwrap(), fname.filename);
		}
		if filename_regex.is_some() && from_lang.is_none() {
			println!("Ignoring file not matching given regex {}", fname.filename);
			continue;
		}
		match fname.reason {
			FilenameChangeReason::Add => {
				let fnamef = fname.filename.as_ref();
				let po_map = try!(selfcontained_blob_parser(repo, &tree_new, fnamef, None));
				// we have no old versions
				for (key, val) in po_map.iter() {
					res.push(QuestionSubject {
						commit_id: commit_id.to_string(),
						from_filename: fnamef.to_string(),
						orig: key.to_string(),
						old: None,
						new: val.to_string(),
						oldtrans: "<no old version available>".to_string(),
						newtrans: trans.translate(val, from_lang),
					});
				}
			},
			FilenameChangeReason::Modify => {
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
						old: match oldval { Some(v) => Some(v.clone()), None => None},
						new: val.to_string(),
						oldtrans: match oldval {
							Some(v) => trans.translate(v, from_lang),
							None => "?????".to_string()},
						newtrans: trans.translate(val, from_lang),
					});
				}
			},
			FilenameChangeReason::Delete => {
				// do nothing here, perhaps notify...
			},
		}
	}
	return Ok(res);
}


enum FilenameChangeReason {
	Add,
	Modify,
	Delete,
}

impl fmt::Display for FilenameChangeReason {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match *self {
			FilenameChangeReason::Add => write!(f, "add"),
			FilenameChangeReason::Modify => write!(f, "modify"),
			FilenameChangeReason::Delete => write!(f, "delete"),
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
			'A' => FilenameChangeReason::Add,
			'D' => FilenameChangeReason::Delete,
			'M' => FilenameChangeReason::Modify,
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
