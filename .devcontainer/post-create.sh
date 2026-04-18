#!/usr/bin/env bash
set -euxo pipefail

# Named volumes are created root-owned by Docker on first mount, which blocks
# the non-root `vscode` user from writing to them. Re-chowning is idempotent.
sudo chown -R vscode:vscode "$HOME/.claude" "$HOME/.bundle"

# Claude Code CLI (native installer). Installs a standalone binary to
# ~/.local/bin/claude with no Node runtime dependency. The installer is
# idempotent, so re-running on an existing container is safe.
curl -fsSL https://claude.ai/install.sh | bash

# Ruby tooling
gem install bundler --conservative
if [ -f Gemfile ]; then
  bundle install
fi

# Wire GITHUB_TOKEN (loaded from .devcontainer/.env) into gh + git.
# gh automatically picks up GITHUB_TOKEN / GH_TOKEN; this also makes git
# authenticate to github.com transparently via the gh credential helper.
if [ -n "${GITHUB_TOKEN:-}" ]; then
  gh auth setup-git
fi

# Avoid "dubious ownership" when the host bind-mounts the repo.
git config --global --add safe.directory "$(pwd)"
