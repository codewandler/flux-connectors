op github-user-repo-list(visibility: String, affiliation: String, type: String, sort: String, direction: String, per_page: Number, page: Number) -> Any
  description "List the repositories the authenticated user can access, filtered by visibility, affiliation and type"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "https://api.github.com"
  url = fmt("{base}/user/repos")
  Accept = "application/vnd.github+json"
  response = http.request(headers: { Accept }, method: "GET", query: { affiliation, direction, page, per_page, sort, type, visibility }, url)
  return response
