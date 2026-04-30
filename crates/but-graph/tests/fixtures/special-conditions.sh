#!/usr/bin/env bash

source "${BASH_SOURCE[0]%/*}/shared.sh"

git init shallow-clone-depth-2-source
(cd shallow-clone-depth-2-source
  for idx in $(seq 4); do
    commit "commit $idx"
  done
)
git clone --depth 2 "file://$PWD/shallow-clone-depth-2-source" shallow-clone-depth-2

git init shallow-workspace-source
(
  cd shallow-workspace-source
  commit M1
  commit M2
  commit M3
  commit M4
  git checkout -b A
  commit A1
  create_workspace_commit_once A
)

git clone --depth 3 --no-single-branch \
  "file://$PWD/shallow-workspace-source" \
  shallow-workspace-boundary-below-lower-bound
(
  cd shallow-workspace-boundary-below-lower-bound
  git branch A origin/A
  git branch main origin/main
)

git clone --depth 2 \
  "file://$PWD/shallow-workspace-source" \
  shallow-workspace-boundary-in-workspace
(
  cd shallow-workspace-boundary-in-workspace
  git branch A HEAD^
)
