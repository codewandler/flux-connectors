op github-org-list(per_page: Number, page: Number) -> Any
  description "List the organisations the authenticated user belongs to, with bounded integer pagination"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "https://api.github.com"
  url = fmt("{base}/user/orgs")
  Accept = "application/vnd.github+json"
  response = http.request(headers: { Accept }, method: "GET", query: { page, per_page }, url)
  return response
