op github-commit-list(owner: String, repo: String, per_page: Number, page: Number) -> Any
  description "List commits in a repository with bounded integer pagination"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "https://api.github.com"
  url = fmt("{base}/repos/{owner}/{repo}/commits")
  Accept = "application/vnd.github+json"
  response = http.request(headers: { Accept }, method: "GET", query: { page, per_page }, url)
  return response
