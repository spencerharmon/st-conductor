//! Tiny YAML session file for st-conductor.
//!
//! Persisted as `<nsm_path>/conductor.yaml`. See `plan.org` "NSM" section.

use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Session {
	pub numerator: u16,
	pub denominator: u16,
	pub tempo: u16,
}

fn session_file(path: &str) -> PathBuf {
	// NSM hands us a *prefix*. Existing st-loop uses it as a directory
	// (creates it if missing and writes files inside). Follow the same
	// convention here for consistency: one directory per NSM client.
	let mut p = PathBuf::from(path);
	if !p.exists() {
		let _ = std::fs::create_dir_all(&p);
	}
	p.push("conductor.yaml");
	p
}

pub fn load(path: &str) -> Result<Option<Session>, Box<dyn std::error::Error>> {
	let p = session_file(path);
	if !p.exists() {
		return Ok(None);
	}
	let f = File::open(&p)?;
	let s: Session = serde_yaml::from_reader(f)?;
	Ok(Some(s))
}

pub fn save(path: &str, session: &Session) -> Result<(), Box<dyn std::error::Error>> {
	let p = session_file(path);
	let mut f = File::create(&p)?;
	f.write_all(serde_yaml::to_string(session)?.as_bytes())?;
	Ok(())
}

#[cfg(test)]
mod tests {
	use super::*;
	use std::env::temp_dir;

	#[test]
	fn roundtrip() {
		let dir = temp_dir().join(format!("st-conductor-test-{}", std::process::id()));
		let s = Session { numerator: 7, denominator: 8, tempo: 132 };
		save(dir.to_str().unwrap(), &s).unwrap();
		let loaded = load(dir.to_str().unwrap()).unwrap().unwrap();
		assert_eq!(loaded.numerator, 7);
		assert_eq!(loaded.denominator, 8);
		assert_eq!(loaded.tempo, 132);
		std::fs::remove_dir_all(&dir).ok();
	}

	#[test]
	fn missing_returns_none() {
		let dir = temp_dir().join(format!("st-conductor-missing-{}", std::process::id()));
		assert!(load(dir.to_str().unwrap()).unwrap().is_none());
		std::fs::remove_dir_all(&dir).ok();
	}
}
