op zendesk-ticket-field-list -> Any
  description "List the account's ticket field definitions without optional locale, creator, sort or pagination inputs"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://{subdomain}.zendesk.com"
  url = fmt("{base}/api/v2/ticket_fields")
  response = http.request(method: "GET", url)
  return response
