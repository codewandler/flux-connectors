op zendesk-ticket-show(ticket_id: Number) -> Any
  description "Show Ticket"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://{subdomain}.zendesk.com"
  url = fmt("{base}/api/v2/tickets/{ticket_id}")
  response = http.request(method: "GET", url)
  return response
