op github-issue-get(owner: String, repo: String, issue_number: Number) -> Any
  description "Get one issue by number. GitHub treats a pull request as an issue, so a PR number returns that PR's issue view; use github-pull-get for its merge and review state"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://api.github.com"
  url = fmt("{base}/repos/{owner}/{repo}/issues/{issue_number}")
  Accept = "application/vnd.github+json"
  response = http.request(headers: { Accept }, method: "GET", url)
  return response
