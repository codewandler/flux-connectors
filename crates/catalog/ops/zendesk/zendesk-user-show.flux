op zendesk-user-show(user_id: Number) -> Any
  description "Get one Zendesk user by numeric id"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://{subdomain}.zendesk.com"
  url = fmt("{base}/api/v2/users/{user_id}")
  response = http.request(method: "GET", url)
  return response
