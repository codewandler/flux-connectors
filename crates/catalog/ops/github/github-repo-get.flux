op github-repo-get(owner: String, repo: String) -> Any
  description "Get one repository's metadata, including its default branch, visibility and permissions"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://api.github.com"
  url = fmt("{base}/repos/{owner}/{repo}")
  response = http.request(method: "GET", url)
  return response
