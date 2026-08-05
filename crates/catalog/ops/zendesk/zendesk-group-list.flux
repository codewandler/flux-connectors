op zendesk-group-list -> Any
  description "List the account's Zendesk groups without exposing optional filters or pagination"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "https://{subdomain}.zendesk.com"
  url = fmt("{base}/api/v2/groups")
  response = http.request(method: "GET", url)
  return response
