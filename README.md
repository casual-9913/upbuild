# upbuild

`upbuild` is an interactive Rust CLI framework for selecting YAML/JSON build profiles, cloning/updating Git repositories, and building/deploying Docker workloads.

## Basic usage

```bash
cargo run -- -d ./examples
cargo run -- --set_default_dir ./examples
cargo run --
```

## Menu mapping

| Internal function | CLI label |
|---|---|
| `repo_clone` | Clone Repository |
| `repo_status` | Repository Status |
| `repo_update` | Update Repository |
| `docker_build` | Build Image |
| `docker_container` | Deploy Container |
| `update_n_build` | Run All |

## Config schema

```yaml
name: optional-name

git:
  remote: https://github.com/org/repo.git
  local_dir: /optional/existing/local/repo
  save_dir: /optional/parent/clone/directory

docker:
  file_src: Dockerfile
  build_cmd: docker build -t image:tag -f Dockerfile .
  container_cmd: docker run --name name -it --rm image:tag
```

If `git.local_dir` is absent, the local repository directory is derived from `git.save_dir` plus the remote repository name. If `git.save_dir` is also absent, `repos/<repo-name>` is used relative to the current working directory.
