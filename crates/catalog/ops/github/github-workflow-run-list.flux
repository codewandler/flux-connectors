op github-workflow-run-list(owner: String, repo: String, per_page: Number, page: Number) -> Any
  description "List a repository's GitHub Actions workflow runs with bounded integer pagination"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://api.github.com"
  url = fmt("{base}/repos/{owner}/{repo}/actions/runs")
  Accept = "application/vnd.github+json"
  response = http.request(headers: { Accept }, method: "GET", query: { page, per_page }, url)
  return response
