op zendesk-ticket-audit-list(ticket_id: Number) -> Any
  description "List the read-only audit history for one ticket, including field changes, comments and notifications"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "https://{subdomain}.zendesk.com"
  url = fmt("{base}/api/v2/tickets/{ticket_id}/audits")
  response = http.request(method: "GET", url)
  return response
