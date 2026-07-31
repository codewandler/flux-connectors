op okta-user-get(user_id: String) -> Any
  description "Get one user's full record, including their lifecycle status and profile attributes. Accepts the user's Okta id or their login, so a caller holding an email address does not need to list first"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://{domain}/api/v1"
  url = fmt("{base}/users/{user_id}")
  response = http.request(method: "GET", url)
  return response
