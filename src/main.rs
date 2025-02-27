use std::collections::HashSet;
use std::env;
use std::path::Path;
use std::process::{Command, Output};
use std::str;
use regex::Regex;

struct NameChange {
    commit: String,
    date: String,
    author: String,
    name: String,
}

fn run_command(args: &[&str]) -> Result<Output, String> {
    let output = Command::new(args[0])
        .args(&args[1..])
        .output()
        .map_err(|e| format!("Failed to execute command: {}", e))?;
    
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if !stderr.is_empty() {
            return Err(format!("Command error: {}", stderr));
        }
    }
    
    Ok(output)
}

fn bytes_to_string(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).to_string()
}

fn main() -> Result<(), String> {
    let args: Vec<String> = env::args().collect();
    let repo_dir = if args.len() > 1 {
        Path::new(&args[1])
    } else {
        Path::new(".")
    };
    
    env::set_current_dir(repo_dir).map_err(|e| format!("Cannot change to directory {:?}: {}", repo_dir, e))?;
    
    let git_check = run_command(&["git", "rev-parse", "--is-inside-work-tree"]);
    if git_check.is_err() {
        return Err("Not in a git repository.".to_string());
    }
    
    println!("Fetching commits that modified the Makefile...");
    let commits_output = run_command(&["git", "log", "--pretty=format:%H", "--", "Makefile"])?;
    let commits_str = bytes_to_string(&commits_output.stdout);
    
    let commits: Vec<String> = commits_str.lines().map(|s| s.to_string()).collect();
    println!("Found {} commits to process", commits.len());
    
    let name_regex = Regex::new(r"^\s*NAME\s*[?:]?=\s*(.+)\s*$").unwrap();
    let version_name_regex = Regex::new(r"^\s*VERSION_NAME\s*[?:]?=\s*(.+)\s*$").unwrap();
    let quote_regex = Regex::new(r#"["'](.+)["']"#).unwrap();
    let whitespace_regex = Regex::new(r"^\s+|\s+$").unwrap();
    
    let mut name_history: HashSet<String> = HashSet::new();
    let mut name_changes: Vec<NameChange> = Vec::new();
    
    println!("Processing commits...");
    for (i, commit) in commits.iter().rev().enumerate() {
        if i % 100 == 0 {
            println!("Progress: {} / {}", i, commits.len());
        }
        
        let date_output = run_command(&["git", "show", "-s", "--format=%ci", commit])?;
        let date = bytes_to_string(&date_output.stdout).trim().to_string();
        
        let author_output = run_command(&["git", "show", "-s", "--format=%an", commit])?;
        let author = bytes_to_string(&author_output.stdout).trim().to_string();
        
        let makefile_output = Command::new("git")
            .args(&["show", &format!("{}:Makefile", commit)])
            .output();
        
        if makefile_output.is_err() {
            continue;
        }
        
        let unwrapped_output = makefile_output.unwrap();
        let makefile_content = bytes_to_string(&unwrapped_output.stdout);
        
        let mut name = None;
        
        for line in makefile_content.lines() {
            if let Some(captures) = name_regex.captures(line) {
                name = captures.get(1).map(|m| m.as_str().to_string());
                break;
            } else if let Some(captures) = version_name_regex.captures(line) {
                name = captures.get(1).map(|m| m.as_str().to_string());
                break;
            }
        }
        
        let name = match name {
            Some(n) => n,
            None => continue,
        };
        
        let name = if let Some(captures) = quote_regex.captures(&name) {
            captures.get(1).map_or(name.clone(), |m| m.as_str().to_string())
        } else {
            name
        };
        
        let name = whitespace_regex.replace_all(&name, "").to_string();
        
        if name.is_empty() || name_history.contains(&name) {
            continue;
        }
        
        name_history.insert(name.clone());
        
        name_changes.push(NameChange {
            commit: commit.to_string(),
            date: date.to_string(),
            author: author.to_string(),
            name: name.clone(),
        });
    }
    
    println!("\nLinux Kernel NAME History:");
    println!("=========================");
    
    for change in &name_changes {
        println!("{} - {}", change.date, change.name);
        println!("  Commit: {}", change.commit);
        println!("  Author: {}", change.author);
        println!();
    }
    
    println!("Total unique names found: {}", name_history.len());
    
    Ok(())
}
