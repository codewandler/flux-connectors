op gitlab-project-list(search: String, membership: Bool, owned: Bool, min_access_level: Number, page: Number, per_page: Number) -> Any
  description "List projects the authenticated user can reach, optionally narrowed by a search term — this is how a caller obtains the numeric project id every other operation requires"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "{origin}/api/v4"
  url = fmt("{base}/projects")
  response = http.request(method: "GET", query: { membership, min_access_level, owned, page, per_page, search }, url)
  return response
