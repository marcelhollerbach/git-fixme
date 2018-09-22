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
  git-fixme --stats

Options:
  -h --help      Show this screen.
  --insertion    Insertion of when the FIXME was inserted.
  --file         Only report the file
  --stats        Print how many fixmes accros how many files are there
";

#[derive(Debug, Deserialize)]
struct Args {
    flag_insertion : bool,
    flag_file : bool,
    flag_stats : bool
}

struct Stats {
  fixmes : usize,
  files : usize
}

enum DirAction {
  Enter(std::path::PathBuf),
  Check(std::path::PathBuf),
  Nothing(bool),
}

struct Global <'a> {
  args :  &'a Args,
  repo : &'a git2::Repository,
  matching_keys : &'a Vec<&'a str>
}

fn path_to_repository_local(path : &std::path::Path, repo : &git2::Repository) -> Option<std::path::PathBuf> {
  //this gets the repository root directory
  let repo_path = repo.path().parent().unwrap();
  if !path.starts_with(repo_path) {
    return None;
  }
  return Some(path.strip_prefix(repo_path).unwrap().to_path_buf());
}

fn line_is_fixme(global : &Global, content : &String) -> bool {

  for key in global.matching_keys {
    if content.contains(key) {
      return true;
    }
  }

  return false;
}

fn print_insertion(contents : &String, repo : &git2::Repository, path: &std::path::PathBuf, line_number : usize)
{
  let mut opts = BlameOptions::new();
  let blame = repo.blame_file(&path_to_repository_local(path, repo).unwrap(), Some(&mut opts)).unwrap();
  let hunk = blame.get_line(line_number).unwrap();

  println!("{}:{} @ {}", path.display(), contents, hunk.final_commit_id());
}

fn print_only_files(_contents : &String, path: &std::path::PathBuf, _line_number : usize)
{
  println!("{}", path.display());
}

fn print_default(contents : &String, path: &std::path::PathBuf, line_number : usize)
{
  println!("{}:{} {}", path.display(), line_number, contents);
}

fn handle_file(global : &Global, path: &std::path::PathBuf) -> Result<usize, std::io::Error> {
  let file = try!(File::open(path));
  let mut buf_reader = BufReader::new(file);
  let mut contents = String::from("a");
  let mut line_number = 1;
  let mut hit_counter = 0;

  while contents.len() > 0 {
    contents.clear();
    let reader = buf_reader.read_line(&mut contents);
    if reader.is_err() {
      let error = reader.unwrap_err();
      if error.kind() == std::io::ErrorKind::InvalidData {
        return Ok(0);
      }
      return Err(error);
    }
    if line_is_fixme(global, &contents) {
      //get rid of \n and print this
      contents.pop();
      hit_counter = hit_counter + 1;
      if global.args.flag_file {
        print_only_files(&contents, path, line_number);
        break;
      } else if global.args.flag_insertion {
        print_insertion(&contents, global.repo, path, line_number);
      } else if global.args.flag_stats {
        //nop here
      } else {
        print_default(&contents, path, line_number);
      }
    }
    line_number = line_number + 1;
  }

  Ok(hit_counter)
}

fn generate_action(global : &Global,entry : DirEntry) -> DirAction {
   let path = entry.path();
   let is_dir = entry.metadata().map(|data| data.is_dir());
   let ignored = global.repo.is_path_ignored(path.as_path());

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
   let index = global.repo.index();

   if !index.is_ok() {
     return DirAction::Nothing(false);
   }

   let index_path = index.unwrap().get_path(&path_to_repository_local(&path, global.repo).unwrap(), 0);

   if index_path.is_some() {
     return DirAction::Check(path);
   } else {
      return DirAction::Nothing(false);
   }


}

fn iterate_directory(global : &Global, p : &std::path::Path, stats : &mut Stats) -> Result<(), ()> {
    let directory = fs::read_dir(p).unwrap();
    let mut error = false;

    for entry in directory {
      if entry.is_err() {
        error = true;
        continue;
      }
      let file = entry.unwrap();
      match generate_action(global, file) {
        DirAction::Enter(path) => {
          let iteration_result = iterate_directory(global, &path, stats);
          if iteration_result.is_err() {
            error = true;
          }
        },
        DirAction::Check(path) => {
          let res = handle_file(global, &path);
          if res.is_err() {
            println!("{}:{}", path.display(), res.unwrap_err());
            error = true;
          } else {
            let fixmes = res.unwrap();
            stats.fixmes = stats.fixmes + fixmes;
            if fixmes > 0 {
              stats.files = stats.files + 1;
            }
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
  let setting : String = match env::var("GIT_FIXME_KEYS") {
    Ok(v) => v,
    _ => String::from("FIXME"),
  };
  let keys : Vec<&str> = setting.split(':').collect();
  let global = Global { args : args, repo : &repo, matching_keys : &keys};
  let mut stats = Stats {fixmes : 0, files : 0 };

  let result = match iterate_directory(&global, cwd, &mut stats) {
    Ok(()) => Ok(()),
    Err(()) => Err(git2::Error::from_str("Some errors happened")),
  };

  if args.flag_stats {
    println!("{} {}", stats.files, stats.fixmes);
  }

  result
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
