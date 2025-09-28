Grumpy
======

A GitHub action to detect and mark spam pull requests from forked repositories. Test

Rules
-----

A pull request is considered as spam if it's coming from a forked repository and meets at least one of following criterias:

* The user registered in the last 6 months and has only forked repositories
* The PR is changing documentation insignificantly
* The PR consists of additions and deletions in a single file only

Installation
------------

Add this step to your workflow file:

``` yaml
- name: Grumpy
  uses: huitseeker/rummelsnuff@master
  with:
    access_token: ${{ secrets.GITHUB_TOKEN }} # Required for GitHub API access
    spam_label: "Spam" # default: "Spam"
    close_spam_prs: "yes" # default: "yes"
```

Configuration
-------------

### Required Inputs

- `access_token`: GitHub access token (use `${{ secrets.GITHUB_TOKEN }}`)

### Optional Inputs

- `spam_label`: Label to apply to spam PRs (default: "Spam")
- `close_spam_prs`: Whether to automatically close spam PRs (default: "yes", set to "no" to disable)

### Example Usage

``` yaml
name: Pull Request Triage
on:
  pull_request_target:
    types: [opened, reopened]

jobs:
  triage:
    runs-on: ubuntu-latest
    steps:
      - name: Grumpy
        uses: huitseeker/rummelsnuff@master
        with:
          access_token: ${{ secrets.GITHUB_TOKEN }}
          spam_label: "Spam"
          close_spam_prs: "yes"
```
