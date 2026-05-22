#![allow(unused)]
use core::panic;
use std::io::stdout;
use std::{env, io};
use std::fmt::format;
use std::process::{Command, Stdio};
use std::fs;

fn main() {
    dotenvy::dotenv().ok();

    let llamacpp_url = env::var("LLAMACPP_REPO")
        .unwrap_or(String::from("URL not found. Check llama.cpp git repo."));
    let local_repo = env::var("LOCAL_REPO")
        .unwrap_or(String::from("Directory not found."));
    
    ///* //uncomment to use all flow
    let cur_stat = check_current_repo_state(&llamacpp_url, &local_repo);
    println!("{cur_stat}\n");

    update_local_copy(&cur_stat, &local_repo);
    

    let backend = String::from("vulkan");
    let filename =  match parse_dockerfile(&local_repo, &backend) {
        Ok(basename) => basename,
        Err(error) => panic!("Cannot find the file")
    };
    copy_n_clean_dockerfile( &local_repo, &filename);

    println!("\nBuild Started..........");
    //build_dockerfile(&local_repo, &filename, "xoverspin3/llama.cpp:vulkan");
    println!("\nBuild Completed..........");

    //*/
    
    let imagename = "xoverspin3/llama.cpp:vulkan";
    let flags = "-ngl 999 -fa 1 --no-mmap --models-dir /local_models_c";
    //create_container(imagename, flags); // main runner

}
//check and update functions for llama.cpp 
fn get_remote_default_branch(url: &str) -> Option<(String, String)> {
    let output = Command::new("git")
        .args(["ls-remote", "--symref", url, "HEAD"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);

    let mut branch: Option<String> = None;
    let mut commit: Option<String> = None;

    for line in stdout.lines() {
        if line.starts_with("ref:") {
            branch = line
                .split_whitespace()
                .nth(1)
                .and_then(|s| s.strip_prefix("refs/heads/"))
                .map(str::to_owned);
        } else {
            let mut parts = line.split_whitespace();
            let hash = parts.next();
            let name = parts.next();

            if name == Some("HEAD") {
                commit = hash.map(str::to_owned);
            }
        }
    }
    Some((branch?, commit?))
}

fn check_current_repo_state(url: &str, local_repo: &str) -> String {
    let (remote_branch, remote_commit) = match get_remote_default_branch(url) {
        Some(output) => output,
        None => (
            String::from("Could not determine the default branch"),
            String::from("Could not determine the current commit")
        )
    };

    let output = Command::new("git")
        .args(["-C", local_repo, "rev-parse", &remote_branch])
        .output()
        .expect("Local commit not found");

    let local_commit = String::from_utf8_lossy(&output.stdout);

    if local_commit == remote_commit {
        return String::from("Local copy is UPDATED.")
    } else {
        println!("Remote Commit: {remote_commit}");
        println!("Local Commit: {local_commit}");
        return String::from("Local copy is OUTDATED")
    }
}

fn update_local_copy(message: &str, local_repo:&str) {
    if message.contains("OUTDATED") {
        println!("Pulling latest changes...");
        let _status = Command::new("git")
            .args(["-C", local_repo, "pull", "--ff-only"])
            .status()
            .expect("Failed to pull the remote repo...");
    }
}

/////////////////////////////////////////////////////////
//dockerfile 
fn parse_dockerfile(src: &str, backend: &str) -> io::Result<String> {
    let mut basename = String::from("");
    let mut src = src.to_string();
    let dvops = format!("/.devops");
    src.push_str(&dvops);
    
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let path = entry.path();

        if !path.is_file() {
            continue;
        }

        let filename = match path.file_name().and_then(|n| n.to_str()) {
            Some(name) => name,
            None => continue,
        };

        if filename.contains(backend) && filename.ends_with(".Dockerfile") {
            basename.push_str(filename);
        }
    }
    Ok(basename)
}

fn copy_n_clean_dockerfile(dst: &str, filename: &str) {
    let mut src = dst.to_string();
    let dvops = format!("/.devops/{}", filename);
    src.push_str(&dvops);
    
    let mut dst = dst.to_string();
    let dst_f = format!("/{}", filename);
    dst.push_str(&dst_f);

    //Copy
    let byte_sz = fs::copy(src, &dst);
    println!("Copied {filename} to {dst}.");

    //Read
    let content = fs::read_to_string(&dst).expect("Cannot read the file.");

    let mut result = Vec::new();
    let mut inserted = false;

    for line in content.lines() {
        if !inserted && line.trim() == "ENTRYPOINT [\"/app/tools.sh\"]".to_string() {
            result.push("ENV LLAMA_ARG_HOST=0.0.0.0".to_string());
            inserted = true;
        }

        if line.trim() == "### Light, CLI only" {
            break;
        }

        result.push(line.to_string());
    }

    let new_content = result.join("\n");
    fs::write(&dst, new_content);
    println!("{filename} has been updated.\n")
}

fn build_dockerfile(src: &str, filename: &str, imagename: &str) {
    let mut src_file = src.to_string();
    let f: String = format!("/{}", filename);
    src_file.push_str(&f);

    let _start_docker = Command::new("systemctl")
        .args(["start", "docker"])
        .status()
        .expect("Docker Engine cannot be started");
    let build_output = Command::new("docker")
        .args(["build", "-t", imagename, "-f", &src_file, src])
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .output()
        .expect("Image build failed.");
}

fn create_container(imagename: &str, flags: &str) {
    let mut args = format!("run --name lcvulk_server \
        --entrypoint /app/llama-server \
        --rm \
        --device /dev/kfd \
        --device /dev/dri \
        --security-opt seccomp=unconfined \
        -p 8080:8080 \
        -v local_models:/local_models_c \
        {imagename} \
        ");

    args.push_str(flags);

    let final_args: Vec<&str> = args.split(" ").collect();

    let _create_container = Command::new("docker")
        .args(final_args)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .output()
        .expect("Container build failed. ");
}

// add cli interface??
//compile??/