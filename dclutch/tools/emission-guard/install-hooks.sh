#!/bin/sh
# Install the emission guard as this checkout's pre-push hook. Opt-in, local,
# and reversible -- run it yourself; nothing installs it for you.
#
# WHAT IT CHANGES, stated plainly before it changes anything: one repository-
# local git setting, `core.hooksPath`, pointing at .githooks/ in this
# checkout. It writes to this repository's .git/config only. It does not touch
# any file in your home directory and it does not change git's global
# configuration.
#
# THE CATCH YOU NEED TO KNOW, because it is not obvious: a repository-local
# `core.hooksPath` OVERRIDES a global one. If you have global hooks installed
# (lefthook shims and the like), they will stop running IN THIS REPOSITORY
# while this is set. That is why this is a script you run deliberately rather
# than something a lane switched on for you.
#
# To undo:  git config --unset core.hooksPath
# To check: git config --get core.hooksPath

set -eu

repository_dir=$(git rev-parse --show-toplevel)
hooks_dir="$repository_dir/.githooks"

existing=$(git config --get core.hooksPath || echo '')
global=$(git config --global --get core.hooksPath || echo '')

if [ -n "$global" ] && [ -z "$existing" ]; then
  echo "note: you have a GLOBAL core.hooksPath ($global)."
  echo "      Installing here overrides it for this repository only."
  echo "      Any global hook (lefthook shims etc.) stops running in this checkout."
  printf 'continue? [y/N] '
  read -r reply
  case "$reply" in
    y | Y) ;;
    *) echo "aborted; nothing changed"; exit 1 ;;
  esac
fi

mkdir -p "$hooks_dir"
cp "$repository_dir/tools/emission-guard/pre-push" "$hooks_dir/pre-push"
chmod +x "$hooks_dir/pre-push"
git config core.hooksPath .githooks

echo "installed: .githooks/pre-push (core.hooksPath = .githooks, this repository only)"
echo "skip one push with: SKIP_EMISSION_GUARD=1 git push"
echo "uninstall with:     git config --unset core.hooksPath"
