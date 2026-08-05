op okta-group-list(limit: Number) -> Any
  description "List groups in the Okta org. Returns a JSON array of group objects. A group is how Okta grants application access in bulk, so this is the set a membership question is asked against"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "https://{domain}/api/v1"
  url = fmt("{base}/groups")
  response = http.request(method: "GET", query: { limit }, url)
  return response
