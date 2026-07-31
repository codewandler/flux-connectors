op okta-user-list(limit: Number) -> Any
  description "List users in the Okta org. Returns a JSON array of user objects, most recently created first. Without a filter this is the org's active user population, so use `limit` to bound it — see okta-user-get to read one user by id or login"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://{domain}/api/v1"
  url = fmt("{base}/users")
  sep = "?"
  when limit
    url = fmt("{url}{sep}limit={limit}")
  response = http.request(method: "GET", url)
  return response
