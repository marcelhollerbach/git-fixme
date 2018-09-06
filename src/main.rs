extern crate git2;
extern crate docopt;

#[macro_use]
extern crate serde_derive;

use std::env;
use docopt::Docopt;
use git2::{Repository, BlameOptions};
use std::fs::{self, DirEntry, File};
use std::io::{BufReader, BufRead};

const USAGE: &'static str = "
git-fixme.

Usage:
  git-fixme
  git-fixme (-h | --help)
  git-fixme --insertion
  git-fixme --file

Options:
  -h --help      Show this screen.
  --insertion    Insertion of when the FIXME was inserted.
  --file         Only report the file
";

#[derive(Debug, Deserialize)]
struct Args {
    flag_insertion : bool,
    flag_file : bool
}

enum DirAction {
  Enter(std::path::PathBuf),
  Check(std::path::PathBuf),
  Nothing(bool),
}

fn path_to_repository_local(path : &std::path::Path, repo : &git2::Repository) -> Option<std::path::PathBuf> {
  //this gets the repository root directory
  let repo_path = repo.path().parent().unwrap();
  if !path.starts_with(repo_path) {
    return None;
  }
  return Some(path.strip_prefix(repo_path).unwrap().to_path_buf());
}

fn line_patches(content : &String) -> bool {

  let setting : String = match env::var("GIT_FIXME_KEYS") {
    Ok(v) => v,
    _ => String::from("FIXME"),
  };

  let keys : Vec<&str> = setting.split(':').collect();

  for key in keys {
    if content.contains(key) {
      return true;
    }
  }

  return false;
}

fn handle_file(args : &Args, repo : &git2::Repository, path: &std::path::PathBuf) -> Result<(), std::io::Error> {
  let file = try!(File::open(path));
  let mut buf_reader = BufReader::new(file);
  let mut contents = String::from("a");
  let mut i = 0;

  while contents.len() > 0 {
    contents.clear();
    let reader = buf_reader.read_line(&mut contents);
    if reader.is_err() {
      return Err(reader.unwrap_err());
    }
    if line_patches(&contents) {
      print!("{}", path.display());

      //only report a file once
      if args.flag_file {
        println!("");
        break;
      }

      //get rid of \n and print this
      contents.pop();
      print!(":{}", contents);

      //either add the revision where the line was changed, or add a \n
      if args.flag_insertion {
        let mut opts = BlameOptions::new();
        let blame = repo.blame_file(&path_to_repository_local(path, repo).unwrap(), Some(&mut opts)).unwrap();
        let hunk = blame.get_line(i + 1).unwrap();
        println!(" @ {}", hunk.final_commit_id());
      } else {
        println!("");
      }
      i = i + 1;
    }
  }
  Ok(())
}

fn generate_action(repo : &git2::Repository,entry : DirEntry) -> DirAction {
   let path = entry.path();
   let is_dir = entry.metadata().map(|data| data.is_dir());
   let ignored = repo.is_path_ignored(path.as_path());

   if is_dir.is_err() {
     println!("{}", is_dir.err().unwrap());
     return DirAction::Nothing(true);
   }

   if ignored.is_err() {
     println!("{}", ignored.err().unwrap());
     return DirAction::Nothing(true);
   }

   if ignored.unwrap() {
     return DirAction::Nothing(false);
   }

   if is_dir.unwrap() {
     return DirAction::Enter(path)
   }

   //check if the file is in the index
   let index = repo.index();

   if !index.is_ok() {
     return DirAction::Nothing(false);
   }

   let index_path = index.unwrap().get_path(&path_to_repository_local(&path, repo).unwrap(), 0);

   if index_path.is_some() {
     return DirAction::Check(path);
   } else {
      return DirAction::Nothing(false);
   }


}

fn iterate_directory(args : &Args, repo : &git2::Repository, p : &std::path::Path) -> Result<(), ()> {
    let directory = fs::read_dir(p).unwrap();
    let mut error = false;

    for entry in directory {
      if entry.is_err() {
        error = true;
        continue;
      }
      let file = entry.unwrap();
      match generate_action(repo, file) {
        DirAction::Enter(path) => {
          let iteration_result = iterate_directory(args, repo, &path);
          if iteration_result.is_err() {
            error = true;
          }
        },
        DirAction::Check(path) => {
          let res = handle_file(args, repo, &path);
          if res.is_err() {
            error = true;
          }
        },
        DirAction::Nothing(b) => {
          if b {
            error = true;
          }
          continue
        },
      }
    }
    if error {
      return Err(());
    } else {
      Ok(())
    }
}

fn run(args : &Args) -> Result<(), (git2::Error)> {
  let cwd_buf = env::current_dir().unwrap();
  let cwd = cwd_buf.as_path();
  let repo = try!(Repository::discover(cwd));

  match iterate_directory(args, &repo, &cwd) {
    Ok(()) => Ok(()),
    Err(()) => Err(git2::Error::from_str("Some errors happened")),
  }
}

fn main() {
    let args: Args = Docopt::new(USAGE)
                            .and_then(|d| d.deserialize())
                            .unwrap_or_else(|e| e.exit());

    match run(&args) {
        Ok(()) => {}
        Err(e) => println!("error: {}", e),
    }
}
