op github-pull-files-list(owner: String, repo: String, pull_number: Number, per_page: Number, page: Number) -> Any
  description "List the files changed by one pull request with bounded integer pagination"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://api.github.com"
  url = fmt("{base}/repos/{owner}/{repo}/pulls/{pull_number}/files")
  sep = "?"
  when per_page
    url = fmt("{url}{sep}per_page={per_page}")
    sep = "&"
  when page
    url = fmt("{url}{sep}page={page}")
  Accept = "application/vnd.github+json"
  response = http.request(headers: { Accept }, method: "GET", url)
  return response
