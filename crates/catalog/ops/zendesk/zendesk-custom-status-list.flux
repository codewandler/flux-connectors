op zendesk-custom-status-list -> Any
  description "List the account's custom ticket statuses without optional category, activity or default filters"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "https://{subdomain}.zendesk.com"
  url = fmt("{base}/api/v2/custom_statuses")
  response = http.request(method: "GET", url)
  return response
