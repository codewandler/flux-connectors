op zendesk-ticket-form-list -> Any
  description "List the account's ticket forms without optional visibility, type, brand, locale, sort or pagination inputs"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://{subdomain}.zendesk.com"
  url = fmt("{base}/api/v2/ticket_forms")
  response = http.request(method: "GET", url)
  return response
