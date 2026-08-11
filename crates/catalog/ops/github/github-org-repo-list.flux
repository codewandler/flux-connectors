op github-org-repo-list(org: String, type: String, sort: String, direction: String, per_page: Number, page: Number) -> Any
  description "List an organisation's repositories, filtered by type and ordered, with bounded integer pagination"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "https://api.github.com"
  url = fmt("{base}/orgs/{org}/repos")
  Accept = "application/vnd.github+json"
  response = http.request(headers: { Accept }, method: "GET", query: { direction, page, per_page, sort, type }, url)
  return response
