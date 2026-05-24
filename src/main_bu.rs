use std::{
    env,
    error::Error,
    fmt,
    fs,
    io,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

type AppResult<T> = Result<T, Box<dyn Error>>;

const LIGHT_CLI_MARKER: &str = "### Light, CLI only";
const SERVER_ENTRYPOINT: &str = "ENTRYPOINT [\"/app/llama-server\"]";
const LLAMA_HOST_ENV: &str = "ENV LLAMA_ARG_HOST=0.0.0.0";

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}

fn run() -> AppResult<()> {
    // Keep .env support because the original program already used dotenvy.
    // Required Cargo.toml dependency: dotenvy = "0.15"
    let _ = dotenvy::dotenv();

    let config = AppConfig::from_env()?;

    let repo_state = check_current_repo_state(&config.llamacpp_repo, &config.local_repo)?;
    println!("{repo_state}");

    update_local_copy(&repo_state, &config.local_repo, &config.llamacpp_repo)?;

    let source_dockerfile = parse_dockerfile(&config.local_repo, config.backend.as_str())?;
    println!("Selected Dockerfile: {}", source_dockerfile.display());

    let prepared_dockerfile = copy_n_clean_dockerfile(&config.local_repo, &source_dockerfile)?;
    println!("Prepared Dockerfile: {}", prepared_dockerfile.display());

    println!("\nBuild started...");
    build_dockerfile(&config.local_repo, &prepared_dockerfile, &config.image_name)?;
    println!("Build completed.");

    if config.deploy_container {
        create_container(&config)?;
    } else {
        println!("Container deployment skipped. Set DEPLOY_CONTAINER=true to run it after build.");
    }

    Ok(())
}



#[derive(Debug)]
struct AppConfig {
    llamacpp_repo: String,
    local_repo: PathBuf,
    backend: Backend,
    image_name: String,
    deploy_container: bool,
    container_name: String,
    port_mapping: String,
    model_mount: String,
    llama_args: Vec<String>,
    auto_start_docker: bool,
}

impl AppConfig {
    fn from_env() -> AppResult<Self> {
        let llamacpp_repo = required_env("LLAMACPP_REPO")?;
        let local_repo = PathBuf::from(required_env("LOCAL_REPO")?);

        if !local_repo.is_dir() {
            return Err(boxed_err(format!(
                "LOCAL_REPO does not point to a directory: {}",
                local_repo.display()
            )));
        }

        let backend = Backend::parse(&env::var("BACKEND").unwrap_or_else(|_| "vulkan".to_string()));
        let image_name = env::var("IMAGE_NAME")
            .unwrap_or_else(|_| format!("xoverspin3/llama.cpp:{}", backend.as_str()));

        let llama_args = env::var("LLAMA_ARGS")
            .unwrap_or_else(|_| "-ngl 999 -fa 1 --no-mmap --models-dir /local_models_c".to_string())
            .split_whitespace()
            .map(str::to_owned)
            .collect();

        Ok(Self {
            llamacpp_repo,
            local_repo,
            backend,
            image_name,
            deploy_container: env_bool("DEPLOY_CONTAINER", false),
            container_name: env::var("CONTAINER_NAME").unwrap_or_else(|_| "llamacpp_server".to_string()),
            port_mapping: env::var("PORT_MAPPING").unwrap_or_else(|_| "8080:8080".to_string()),
            model_mount: env::var("MODEL_MOUNT").unwrap_or_else(|_| "local_models:/local_models_c".to_string()),
            auto_start_docker: env_bool("AUTO_START_DOCKER", false),
            llama_args,
        })
    }
}

#[derive(Debug, Clone)]
enum Backend {
    Cpu,
    Vulkan,
    Rocm,
    Cuda,
    Other(String),
}

impl Backend {
    fn parse(raw: &str) -> Self {
        match raw.trim().to_ascii_lowercase().as_str() {
            "cpu" => Self::Cpu,
            "vulkan" => Self::Vulkan,
            "rocm" | "amd" => Self::Rocm,
            "cuda" | "nvidia" => Self::Cuda,
            other => Self::Other(other.to_string()),
        }
    }

    fn as_str(&self) -> &str {
        match self {
            Self::Cpu => "cpu",
            Self::Vulkan => "vulkan",
            Self::Rocm => "rocm",
            Self::Cuda => "cuda",
            Self::Other(value) => value.as_str(),
        }
    }

    fn docker_run_args(&self) -> Vec<String> {
        match self {
            Self::Cpu => Vec::new(),
            Self::Vulkan| Self::Rocm => vec![
                "--device".into(),
                "/dev/kfd".into(),
                "--device".into(),
                "/dev/dri".into(),
                "--security-opt".into(),
                "seccomp=unconfined".into(),
            ],
            Self::Cuda => vec!["--gpus".into(), "all".into()],
            Self::Other(_) => Vec::new(),
        }
    }
}

#[derive(Debug)]
struct RemoteHead {
    branch: String,
    commit: String,
}

#[derive(Debug)]
enum RepoState {
    UpToDate { branch: String, commit: String },
    Outdated {
        branch: String,
        local_commit: String,
        remote_commit: String,
    },
}

impl fmt::Display for RepoState {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UpToDate { branch, commit } => write!(
                formatter,
                "Local copy is up to date. branch={branch}, commit={commit}"
            ),
            Self::Outdated {
                branch,
                local_commit,
                remote_commit,
            } => write!(
                formatter,
                "Local copy is outdated. branch={branch}\nlocal ={local_commit}\nremote={remote_commit}"
            ),
        }
    }
}
///////////////////////////
fn get_remote_default_branch(url: &str) -> AppResult<RemoteHead> {
    let mut command = Command::new("git");
    command.args(["ls-remote", "--symref", url, "HEAD"]);

    let stdout = command_stdout(&mut command, "query remote default branch")?;

    let mut branch = None;
    let mut commit = None;

    for line in stdout.lines() {
        if line.starts_with("ref:") {
            branch = line
                .split_whitespace()
                .nth(1)
                .and_then(|value| value.strip_prefix("refs/heads/"))
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

    let branch = branch.ok_or_else(|| boxed_err("could not determine remote default branch"))?;
    let commit = commit.ok_or_else(|| boxed_err("could not determine remote HEAD commit"))?;

    Ok(RemoteHead { branch, commit })
}

fn check_current_repo_state(url: &str, local_repo: &Path) -> AppResult<RepoState> {
    ensure_git_worktree(local_repo)?;

    let remote = get_remote_default_branch(url)?;

    let mut command = Command::new("git");
    command.arg("-C").arg(local_repo).args(["rev-parse", "HEAD"]);
    let local_commit = command_stdout(&mut command, "read local HEAD commit")?
        .trim()
        .to_string();

    if local_commit == remote.commit {
        Ok(RepoState::UpToDate {
            branch: remote.branch,
            commit: local_commit,
        })
    } else {
        Ok(RepoState::Outdated {
            branch: remote.branch,
            local_commit,
            remote_commit: remote.commit,
        })
    }
}

fn update_local_copy(state: &RepoState, local_repo: &Path, remote_url: &str) -> AppResult<()> {
    let branch = match state {
        RepoState::UpToDate { .. } => {
            println!("No repository update needed.");
            return Ok(());
        }
        RepoState::Outdated { branch, .. } => branch,
    };

    ensure_clean_worktree(local_repo)?;

    println!("Pulling latest changes from remote branch '{branch}'...");

    let mut fetch = Command::new("git");
    fetch
        .arg("-C")
        .arg(local_repo)
        .args(["fetch", "--prune", remote_url, branch]);
    command_status(&mut fetch, "fetch remote repository")?;

    let mut merge = Command::new("git");
    merge
        .arg("-C")
        .arg(local_repo)
        .args(["merge", "--ff-only", "FETCH_HEAD"]);
    command_status(&mut merge, "fast-forward local repository")?;

    Ok(())
}

fn command_status(command: &mut Command, action: &str) -> AppResult<()> {
    println!("$ {command:?}");

    let status = command
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .map_err(|error| boxed_err(format!("failed to {action}: {error}")))?;

    if status.success() {
        Ok(())
    } else {
        Err(boxed_err(format!(
            "{action} failed with status {status}"
        )))
    }
}

fn parse_dockerfile(local_repo: &Path, backend: &str) -> AppResult<PathBuf> {
    let devops_dir = local_repo.join(".devops");

    if !devops_dir.is_dir() {
        return Err(boxed_err(format!(
            "llama.cpp .devops directory not found: {}",
            devops_dir.display()
        )));
    }

    let needle = backend.to_ascii_lowercase();
    let mut matches = Vec::new();

    for entry in fs::read_dir(&devops_dir)? {
        let path = entry?.path();

        if !path.is_file() {
            continue;
        }

        let Some(filename) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };

        let lower_filename = filename.to_ascii_lowercase();
        if lower_filename.ends_with(".dockerfile") && lower_filename.contains(&needle) {
            matches.push(path);
        }
    }

    matches.sort();

    match matches.len() {
        0 => Err(boxed_err(format!(
            "no Dockerfile found for backend '{backend}' under {}",
            devops_dir.display()
        ))),
        1 => Ok(matches.remove(0)),
        _ => Err(boxed_err(format!(
            "multiple Dockerfiles match backend '{backend}': {}",
            matches
                .iter()
                .map(|path| path.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        ))),
    }
}

fn copy_n_clean_dockerfile(local_repo: &Path, source_dockerfile: &Path) -> AppResult<PathBuf> {
    let filename = source_dockerfile
        .file_name()
        .ok_or_else(|| boxed_err("source Dockerfile has no filename"))?;

    let destination = local_repo.join(filename);

    fs::copy(source_dockerfile, &destination).map_err(|error| {
        boxed_err(format!(
            "failed to copy {} to {}: {error}",
            source_dockerfile.display(),
            destination.display()
        ))
    })?;

    let content = fs::read_to_string(&destination).map_err(|error| {
        boxed_err(format!(
            "failed to read copied Dockerfile {}: {error}",
            destination.display()
        ))
    })?;

    let mut result = Vec::new();
    let mut entrypoint_written = false;
    let mut host_env_written = false;

    for line in content.lines() {
        let trimmed = line.trim();

        if trimmed == LIGHT_CLI_MARKER {
            break;
        }

        // Remove old copies so repeated runs are idempotent.
        if trimmed == LLAMA_HOST_ENV {
            continue;
        }

        if trimmed.starts_with("ENTRYPOINT ") {
            if !host_env_written {
                result.push(LLAMA_HOST_ENV.to_string());
                host_env_written = true;
            }

            result.push(SERVER_ENTRYPOINT.to_string());
            entrypoint_written = true;
            continue;
        }

        result.push(line.to_string());
    }

    if !entrypoint_written {
        if !host_env_written {
            result.push(LLAMA_HOST_ENV.to_string());
        }
        result.push(SERVER_ENTRYPOINT.to_string());
    }

    fs::write(&destination, format!("{}\n", result.join("\n"))).map_err(|error| {
        boxed_err(format!(
            "failed to write cleaned Dockerfile {}: {error}",
            destination.display()
        ))
    })?;

    Ok(destination)
}

fn build_dockerfile(build_context: &Path, dockerfile: &Path, image_name: &str) -> AppResult<()> {
    ensure_docker_available(env_bool("AUTO_START_DOCKER", false))?;

    let mut command = Command::new("docker");
    command
        .arg("build")
        .arg("-t")
        .arg(image_name)
        .arg("-f")
        .arg(dockerfile)
        .arg(build_context);

    command_status(&mut command, "build Docker image")
}

fn create_container(config: &AppConfig) -> AppResult<()> {
    ensure_docker_available(config.auto_start_docker)?;

    let mut args = vec![
        "run".to_string(),
        "--name".to_string(),
        config.container_name.clone(),
        "--rm".to_string(),
    ];

    args.extend(config.backend.docker_run_args());
    args.extend(["--entrypoint".to_string(), "/app/llama-server".to_string()]);
    args.extend(["-p".to_string(), config.port_mapping.clone()]);
    args.extend(["-v".to_string(), config.model_mount.clone()]);
    args.push(config.image_name.clone());
    args.extend(config.llama_args.iter().cloned());

    let mut command = Command::new("docker");
    command.args(&args);

    command_status(&mut command, "run Docker container")
}

fn ensure_docker_available(auto_start: bool) -> AppResult<()> {
    if docker_info_succeeds()? {
        return Ok(());
    }

    if auto_start {
        let mut command = Command::new("systemctl");
        command.args(["start", "docker"]);
        command_status(&mut command, "start Docker with systemctl")?;

        if docker_info_succeeds()? {
            return Ok(());
        }
    }

    Err(boxed_err(
        "Docker is not available. Start Docker Engine first, or set AUTO_START_DOCKER=true on a systemd Linux host.",
    ))
}

fn docker_info_succeeds() -> AppResult<bool> {
    let status = Command::new("docker")
        .arg("info")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|error| boxed_err(format!("failed to execute docker: {error}")))?;

    Ok(status.success())
}

fn ensure_git_worktree(path: &Path) -> AppResult<()> {
    let mut command = Command::new("git");
    command
        .arg("-C")
        .arg(path)
        .args(["rev-parse", "--is-inside-work-tree"]);

    let output = command_stdout(&mut command, "validate local git repository")?;

    if output.trim() == "true" {
        Ok(())
    } else {
        Err(boxed_err(format!(
            "path is not a git worktree: {}",
            path.display()
        )))
    }
}

fn ensure_clean_worktree(local_repo: &Path) -> AppResult<()> {
    let mut command = Command::new("git");
    command
        .arg("-C")
        .arg(local_repo)
        .args(["status", "--porcelain"]);

    let output = command_stdout(&mut command, "check git worktree status")?;

    if output.trim().is_empty() {
        Ok(())
    } else {
        Err(boxed_err(format!(
            "local repository has uncommitted changes; commit/stash them before updating:\n{output}"
        )))
    }
}

fn command_stdout(command: &mut Command, action: &str) -> AppResult<String> {
    let output = command
        .output()
        .map_err(|error| boxed_err(format!("failed to {action}: {error}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(boxed_err(format!(
            "{action} failed with status {}: {}",
            output.status,
            stderr.trim()
        )));
    }

    String::from_utf8(output.stdout)
        .map_err(|error| boxed_err(format!("{action} returned non-UTF-8 stdout: {error}")))
}

fn env_bool(name: &str, default: bool) -> bool {
    match env::var(name) {
        Ok(value) => matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "y" | "on"
        ),
        Err(_) => default,
    }
}

fn required_env(name: &str) -> AppResult<String> {
    env::var(name)
        .map_err(|_| boxed_err(format!("Required environment variable is missing: {name}")))
}

fn boxed_err(message: impl Into<String>) -> Box<dyn Error> {
    Box::new(io::Error::new(io::ErrorKind::Other, message.into()))
}

