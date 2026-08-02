op zendesk-ticket-recent-list -> Any
  description "List the account's most recently created or updated tickets"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://{subdomain}.zendesk.com"
  url = fmt("{base}/api/v2/tickets/recent")
  response = http.request(method: "GET", url)
  return response
