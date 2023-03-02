use std::path::PathBuf;
use std::process;
use std::str::FromStr;

use clap::{arg, command, Parser};
use dialoguer::console::Term;
use dialoguer::theme::ColorfulTheme;
use dialoguer::{Input, Select};
use glob::{glob, Paths};
use prettytable::format::LineSeparator;
use prettytable::{row, Table};
use regex::Regex;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// File path to read
    #[arg()]
    filepath: String,
}

fn get_files(args: &Args) -> Option<Paths> {
    let pattern = match args.filepath.ends_with("/") {
        true => format!("{}**/*", args.filepath),
        false => format!("{}/**/*", args.filepath),
    };

    match glob(&pattern) {
        Ok(val) => Some(val),
        Err(error) => {
            eprintln!(
                "Failed to parse directory pattern ({}): {error:?}",
                &pattern
            );
            return None;
        }
    }
}

fn get_matched_paths(args: &Args, matcher_regex: Regex) -> Vec<PathBuf> {
    println!("Finding files...");
    get_files(args)
        .unwrap()
        .filter_map(|p| {
            match p {
                Ok(path) => {
                    let path_string = path.as_os_str();
                    let path_string = path_string.to_str().unwrap();
                    match matcher_regex.is_match(&path_string) {
                        true => {
                            // let path: String = path_string.into();
                            Some(path)
                        }
                        false => {
                            // eprintln!("Failed to match {path_string}");
                            None
                        }
                    }
                }
                Err(err) => {
                    eprintln!("Error: {err:?}");
                    None
                }
            }
        })
        .collect()
}

// builds the regex and tries to clean it up
fn get_matcher_regex(matcher_string: &str) -> Result<Regex, regex::Error> {
    let mut matcher_string_temp = matcher_string.clone().to_string();

    if !matcher_string_temp.ends_with("$") {
        matcher_string_temp = format!("{matcher_string_temp}$");
    }
    println!("Creating regex on {matcher_string_temp}");
    Regex::new(&matcher_string_temp)
}

// builds the regex and tries to clean it up
fn get_renamer_regex(renamer_string: &str) -> Result<Regex, String> {
    let renamer_string_temp = renamer_string.clone().to_string();

    println!("Creating renamer regex on {renamer_string_temp}");
    let regex = Regex::new(&renamer_string_temp).map_err(|err| format!("{err:?}"))?;
    if regex.capture_names().len() == 1 {
        return Err("You don't have any capture groups for renaming?".to_string());
    }
    if regex.capture_names().len() > 2 {
        return Err(
            "Sorry, this can only deal with a single capture group at the moment!".to_string(),
        );
    }
    Ok(regex)
}

struct Config {
    pub matcher_string: String,
    pub renamer_string: String,
    pub replacement_string: String,
    pub show_unchanged: bool,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            matcher_string: r".*\.jpeg$".to_string(),
            renamer_string: "(jpeg)".to_string(),
            replacement_string: "jpg".to_string(),
            show_unchanged: true,
        }
    }
}

/// takes the found paths, the base path, matcher regex and replacement string and returns a list of start -> end
fn get_change_pairs(
    paths: Vec<PathBuf>,
    base_path: String,
    matcher_regex: Regex,
    replacement_string: &String,
) -> Vec<(PathBuf, PathBuf)> {
    paths
        .into_iter()
        .map(|path| {
            let path_str = path.to_str().unwrap();
            let path_str: String = path_str.replace(&base_path, "");
            let result = matcher_regex
                .replace_all(&path_str, replacement_string)
                .to_string();
            (path.clone(), PathBuf::from_str(&format!("{base_path}{result}")).unwrap())
        })
        .collect()
}

fn apply_changes(changes: Vec<(PathBuf, PathBuf)>) -> bool {
    changes.iter().for_each(|(source_file, dest_file)| {
        println!("moving {source_file:?} to {dest_file:?}");

        if dest_file.exists() {
           eprintln!("File already exists! Not taking action! {dest_file:?}");
        } else {
            match std::fs::rename(source_file, dest_file) {
                Ok(()) => println!("Ok"),
                Err(err) => eprintln!("Failed to rename: {err:?}"),
            };
        }
    });

    false
}

fn main() {
    let args = Args::parse();

    if get_files(&args).is_none() {
        println!("No files found :(");
        process::exit(1);
    }

    let mut config = Config::default();

    let base_path = match PathBuf::from_str(&args.filepath)
        .unwrap()
        .canonicalize() {
            Ok(val) => val,
            Err(err) => {
                eprintln!("Error finding path: {err:?}");
                process::exit(1);
            }
        };
    let base_path = base_path.to_string_lossy();

    let table_format = prettytable::format::FormatBuilder::new()
        .padding(0, 0)
        .borders('|')
        .separator(
            prettytable::format::LinePosition::Intern,
            LineSeparator::new('-', '+', '|', '|'),
        )
        .column_separator('|')
        .build();

    loop {
        config.matcher_string = match Input::<String>::with_theme(&ColorfulTheme::default())
            .with_prompt("Enter your file-matching regex")
            .with_initial_text(config.matcher_string.clone())
            .interact_text()
        {
            Ok(val) => val,
            Err(err) => {
                eprintln!("Input error: {err:?}");
                config.matcher_string
            }
        };

        let matcher_regex = match get_matcher_regex(&config.matcher_string) {
            Ok(val) => val,
            Err(err) => {
                eprintln!("###################################################");
                eprintln!("Failed to parse matcher regex: {err:?}");
                eprintln!("###################################################");
                continue;
            }
        };


        let matched_paths = get_matched_paths(&args, matcher_regex);
        if matched_paths.is_empty() {
            println!("Didn't match any paths!");
            continue;
        }

        println!("Matched {} paths!", matched_paths.len());
        let first_num = match matched_paths.len() >= 10 {
            true => 10,
            false => matched_paths.len(),
        };
        if matched_paths.len() > 1 {
            println!("First {first_num} paths:");
            matched_paths[0..first_num].iter().for_each(|path| {
                let path_str = path.to_str().unwrap();
                println!("{path_str}");
            });
        } else {
            println!("Matched: {:?}", matched_paths.first().unwrap());
        }

        config.renamer_string = match Input::<String>::with_theme(&ColorfulTheme::default())
            .with_prompt("Enter a regex to grab the bit you want to rename")
            .with_initial_text(config.renamer_string.clone())
            .interact_text()
        {
            Ok(val) => val,
            Err(err) => {
                eprintln!("Input error: {err:?}");
                config.renamer_string
            }
        };

        let renamer_regex = match get_renamer_regex(&config.renamer_string) {
            Ok(val) => val,
            Err(err) => {
                eprintln!("###################################################");
                eprintln!("Failed to parse renamer regex: {err:?}");
                eprintln!("###################################################");
                continue;
            }
        };

        config.replacement_string = match Input::<String>::new()
            .with_prompt("Enter your replacement string")
            .with_initial_text(config.replacement_string.clone())
            .interact_text()
        {
            Ok(val) => val,
            Err(err) => {
                eprintln!("Input error, reusing the previous string. Error: {err:?}");
                config.replacement_string.clone()
            }
        };

        let mut table = Table::new();
        table.set_format(table_format);
        table.set_titles(row![ Fyb => "Original", "Replacement"]);
        let changes = get_change_pairs(
            matched_paths,
            base_path.clone().into(),
            renamer_regex,
            &config.replacement_string,
        );

        changes.iter().for_each(|(path_str, result)| {
            table.add_row(row![
                format!("{base_path}{}", path_str.to_str().unwrap()),
                format!("{base_path}{}", result.to_str().unwrap())
            ]);
        });

        if let Err(err) = table.print_tty(false) {
            println!("Failed to output table: {err:?}");
        };

        let mut menu_items = vec!["Change regexes"];

        let menu_apply = format!("Apply changes to {} files", changes.len());
        menu_items.push(&menu_apply);

        if config.show_unchanged {
            menu_items.push("Hide unchanged files");
        } else {
            menu_items.push("Show unchanged files");
        }
        menu_items.push("Quit without making changes");

        let menu_result = Select::with_theme(&ColorfulTheme::default())
            .items(&menu_items)
            .default(0)
            .interact_on_opt(&Term::stderr())
            .map_err(|err| {
                eprintln!("Menu error: {err:?}");
                -1
            })
            .unwrap();

        match menu_result {
            Some(1) => {
                apply_changes(changes);
            }
            Some(2) => {
                config.show_unchanged = !config.show_unchanged;
                match config.show_unchanged {
                    true => println!("Showing unchanged files"),
                    false => println!("Hiding unchanged files"),
                };
            }
            Some(3) => process::exit(0),
            Some(menu_result) => eprintln!("Selected #{menu_result} {}", menu_items[menu_result]),
            None => eprintln!("?"),
        }
    }
}
