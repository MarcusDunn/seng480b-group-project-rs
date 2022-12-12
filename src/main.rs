use crate::CsvColumnDiffType::{Addition, Deletion, Unknown};
use crate::DeclarationType::{Type, Var};
use crate::ToCsvError::IntoInner;
use chrono::{DateTime, TimeZone, Utc};
use git2::{Commit, Diff, DiffFormat, Error, Repository, Sort};
use rayon::prelude::*;
use regex::Regex;
use serde::Serialize;
use std::fs::File;
use std::path::Path;
use std::thread;
use std::thread::JoinHandle;

const REPOSITORIES: [&str; 15] = [
    "https://github.com/dropwizard/dropwizard",
    "https://github.com/hibernate/hibernate-orm",
    "https://github.com/sofastack/sofa-jraft",
    "https://github.com/SeleniumHQ/selenium",
    "https://github.com/open-telemetry/opentelemetry-java",
    "https://github.com/vsilaev/tascalate-javaflow",
    "https://github.com/shopizer-ecommerce/shopizer",
    "https://github.com/eclipse/eclipse.jdt.ls",
    "https://github.com/elastic/elasticsearch",
    "https://github.com/gradle/gradle",
    "https://github.com/spring-projects/spring-framework",
    "https://github.com/google/error-prone",
    "https://github.com/apache/tomcat",
    "https://github.com/networknt/light-4j",
    "https://github.com/INRIA/spoon",
];

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let java_9_release_day = Utc.with_ymd_and_hms(2018, 3, 20, 0, 0, 0).unwrap();
    let repositories = REPOSITORIES
        .into_par_iter()
        .map(download_or_use_cache)
        .collect::<Result<Vec<_>, _>>()?;

    let threads = repositories
        .into_iter()
        .map(|repository| {
            thread::spawn(move || {
                diffs(&repository, &java_9_release_day).map(|diffs| to_csv(&repository, diffs))
            })
        })
        .collect::<Vec<JoinHandle<_>>>();
    for thread in threads {
        match thread.join() {
            Ok(Ok(Ok(file))) => {
                println!("wrote to {:?}", file)
            }
            Ok(Ok(Err(error))) => {
                println!("failed to write to file {}", error)
            }
            Ok(Err(diffs_error)) => {
                println!("diffs error {}", diffs_error)
            }
            Err(join_error) => {
                println!("join_err {:?}", join_error)
            }
        };
    }
    Ok(())
}

#[derive(Debug, thiserror::Error)]
enum ToCsvError {
    #[error("IoError {0}")]
    Io(#[from] std::io::Error),
    #[error("SerializeError {0}")]
    Serialize(#[from] csv::Error),
    #[error("IntoInnerError")]
    IntoInner,
}

#[derive(Serialize)]
struct CsvColumn<'a> {
    diff_type: CsvColumnDiffType,
    line_content: &'a str,
    declaration_type: DeclarationType,
    indentation: i32,
    seconds_since_epoch: i64,
    commit_hash: String,
    file_name: String,
    project_name: &'a str,
    commiter: Option<&'a str>,
}

#[derive(Serialize)]
enum CsvColumnDiffType {
    Addition,
    Deletion,
    Unknown,
}

#[derive(Serialize)]
enum DeclarationType {
    Var,
    Type,
}

impl From<char> for CsvColumnDiffType {
    fn from(char: char) -> Self {
        match char {
            '+' => Addition,
            '-' => Deletion,
            _ => Unknown,
        }
    }
}

fn to_csv<'a>(
    repository: &'a Repository,
    diffs: impl Iterator<Item=Result<(Commit<'a>, Diff<'a>), Error>>,
) -> Result<File, ToCsvError> {
    let var_declaration =
        Regex::new(r#".*var\s+[_a-zA-Z][_$a-zA-Z0-9]*\s*=.*"#).expect("regex is not valid");
    let declaration =
        Regex::new(r#"[^"<#{]*[_a-zA-Z][_$a-zA-Z0-9<>]*\s+[_a-zA-Z][_$a-zA-Z0-9]*\s*=\s*.*"#)
            .expect("regex is not valid");
    let project_name = repository
        .workdir()
        .expect("repo does not have a working directory")
        .to_str()
        .expect("repo is not valid utf8")
        .split('/')
        .filter(|s| !s.is_empty())
        .last()
        .expect("there were no non-empty segments in the working directory")
        .to_string();
    let file = File::options().create(true).write(true).open(
        project_name.clone() + ".csv",
    )?;
    let mut file_writer = csv::WriterBuilder::new().has_headers(true).from_writer(
        file,
    );
    for result in diffs {
        match result {
            Ok((commit, diff)) => {
                diff.print(DiffFormat::Patch, |delta, _, line| {
                    let line_str = match std::str::from_utf8(line.content()) {
                        Ok(line) => line,
                        Err(_) => return true,
                    };
                    let changed_file = match (delta.new_file().path(), delta.old_file().path()) {
                        (Some(file), _) | (_, Some(file)) => {
                            match file.extension().and_then(|os_str| os_str.to_str()) {
                                Some("java") => file,
                                _ => return true,
                            }
                        }
                        _ => return true,
                    };
                    if declaration.is_match(line_str) {
                        file_writer.serialize(CsvColumn {
                            diff_type: CsvColumnDiffType::from(line.origin()),
                            line_content: line_str,
                            declaration_type: if var_declaration.is_match(line_str) {
                                Var
                            } else {
                                Type
                            },
                            indentation: line_str.chars().filter(|c| c.is_whitespace()).fold(
                                0,
                                |acc, c| {
                                    acc + match c {
                                        ' ' => 1,
                                        '\t' => 4,
                                        _ => 0,
                                    }
                                },
                            ),
                            seconds_since_epoch: commit.time().seconds(),
                            commit_hash: commit.as_object().id().to_string(),
                            file_name: changed_file
                                .file_name()
                                .map(|os_str| os_str.to_str())
                                .flatten()
                                .expect("no file")
                                .to_string(),
                            project_name: &project_name,
                            commiter: commit.committer().name(),
                        })
                            .expect("failed to serialize a CsvColumn");
                    }
                    true
                })
                    .expect("ended printing early");
            }
            Err(error) => {
                eprintln!("diff error {}", error)
            }
        }
    }
    file_writer.into_inner().map_err(|_| IntoInner)
}

#[derive(Debug, thiserror::Error)]
enum DownloadOrUseCacheError {
    #[error("InvalidRemoteUrl")]
    InvalidRemoteUrl,
    #[error("git error {0}")]
    GitError(#[from] Error),
}

fn download_or_use_cache(remote_url: &str) -> Result<Repository, DownloadOrUseCacheError> {
    let name = remote_url
        .split('/')
        .last()
        .ok_or(DownloadOrUseCacheError::InvalidRemoteUrl)?;
    let path = Path::new(name);
    if path.exists() {
        println!("{} already exists, reusing for {}", name, remote_url);
        Ok(Repository::open(name)?)
    } else {
        println!("{} does not exist, cloning {}", name, remote_url);
        let repository = Repository::clone(remote_url, path)?;
        println!("cloned {} into {}", remote_url, name);
        Ok(repository)
    }
}

#[derive(Debug, thiserror::Error)]
enum DiffsError {
    #[error("git error {0}")]
    GitError(#[from] Error),
}

fn diffs<'a, 'b: 'a>(
    repo: &'a Repository,
    since: &'b DateTime<Utc>,
) -> Result<impl Iterator<Item=Result<(Commit<'a>, Diff<'a>), Error>>, DiffsError> {
    let mut revwalk = repo.revwalk()?;
    revwalk.push_head()?;
    revwalk.set_sorting(Sort::TIME)?;
    Ok(revwalk
        .map(|rev| rev.and_then(|oid| repo.find_commit(oid)))
        .filter(|commit| {
            commit
                .as_ref()
                .map(|commit| commit.time().seconds() > since.timestamp())
                .unwrap_or(false)
        })
        .filter_map(|commit| {
            commit
                .map(|commit| commit.parents().next().map(|parent| (commit, parent)))
                .transpose()
        })
        .map(|result| {
            result.and_then(|(commit, parent)| {
                repo.diff_tree_to_tree(
                    commit.tree().ok().as_ref(),
                    parent.tree().ok().as_ref(),
                    None,
                )
                    .map(|diff| (commit, diff))
            })
        }))
}
