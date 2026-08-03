op okta-user-group-list(user_id: String, limit: Number) -> Any
  description "List the groups one user is a member of. This is the direct answer to \"what access does this person have\", because Okta grants application assignments through group membership"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://{domain}/api/v1"
  url = fmt("{base}/users/{user_id}/groups")
  response = http.request(method: "GET", query: { limit }, url)
  return response
