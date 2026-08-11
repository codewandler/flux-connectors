op gitlab-group-list(search: String, min_access_level: Number, page: Number, per_page: Number) -> Any
  description "List the groups the authenticated user is a member of, optionally narrowed by a search term"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "{origin}/api/v4"
  url = fmt("{base}/groups")
  response = http.request(method: "GET", query: { min_access_level, page, per_page, search }, url)
  return response
